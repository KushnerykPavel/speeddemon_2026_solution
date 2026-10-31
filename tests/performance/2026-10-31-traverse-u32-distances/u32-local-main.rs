use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::thread;

struct Graph {
    offsets: Vec<u32>,
    edges: Vec<(u32, u32)>,
    incoming: Vec<u64>,
}

fn read_record<const N: usize>(
    reader: &mut impl BufRead,
    buffer: &mut Vec<u8>,
) -> io::Result<Option<[u32; N]>> {
    buffer.clear();
    if reader.read_until(b'\n', buffer)? == 0 {
        return Ok(None);
    }
    parse_fields(buffer).map(Some)
}

fn parse_fields<const N: usize>(buffer: &[u8]) -> io::Result<[u32; N]> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "expected uint32 fields");
    let mut fields = [0_u32; N];
    let mut position = 0;
    for field in &mut fields {
        while position < buffer.len() && buffer[position].is_ascii_whitespace() {
            position += 1;
        }
        let start = position;
        while position < buffer.len() && buffer[position].is_ascii_digit() {
            *field = field
                .checked_mul(10)
                .and_then(|value| value.checked_add(u32::from(buffer[position] - b'0')))
                .ok_or_else(invalid)?;
            position += 1;
        }
        if position == start || (position < buffer.len() && !buffer[position].is_ascii_whitespace())
        {
            return Err(invalid());
        }
    }
    if buffer[position..]
        .iter()
        .any(|byte| !byte.is_ascii_whitespace())
    {
        return Err(invalid());
    }
    Ok(fields)
}

// The competition target and local development hosts use 64-bit Unix off_t.
unsafe extern "C" {
    fn mmap(
        address: *mut std::ffi::c_void,
        length: usize,
        protection: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut std::ffi::c_void;
    fn munmap(address: *mut std::ffi::c_void, length: usize) -> i32;
}

struct MappedFile {
    address: *mut std::ffi::c_void,
    length: usize,
}

impl MappedFile {
    fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let length = usize::try_from(file.metadata()?.len()).map_err(io::Error::other)?;
        if length > isize::MAX as usize {
            return Err(io::Error::other("input is too large to map"));
        }
        let mut mapping = Self {
            address: std::ptr::null_mut(),
            length: 0,
        };
        if length != 0 {
            // Read-only MAP_PRIVATE mapping. Inputs must not be modified/truncated
            // during loading, as guaranteed by the read-only competition data mount.
            let address = unsafe { mmap(std::ptr::null_mut(), length, 1, 2, file.as_raw_fd(), 0) };
            if address as isize == -1 {
                return Err(io::Error::last_os_error());
            }
            if address.is_null() {
                unsafe {
                    munmap(address, length);
                }
                return Err(io::Error::other("null mapping address"));
            }
            mapping = Self { address, length };
        }
        Ok(mapping)
    }

    fn bytes(&self) -> &[u8] {
        if self.length == 0 {
            return &[];
        }
        // The mapping stays alive for this borrow; all scoped workers finish
        // before read_graph returns and Drop unmaps the input.
        unsafe { std::slice::from_raw_parts(self.address.cast(), self.length) }
    }
}

impl Drop for MappedFile {
    fn drop(&mut self) {
        if self.length != 0 {
            unsafe {
                munmap(self.address, self.length);
            }
        }
    }
}

fn line_chunks(bytes: &[u8], workers: usize) -> Vec<&[u8]> {
    let workers = workers.clamp(1, 12);
    let mut chunks = Vec::with_capacity(workers);
    let mut start = 0;
    for part in 1..=workers {
        let mut end = (bytes.len() / workers * part).max(start);
        if part == workers {
            end = bytes.len();
        }
        while end < bytes.len() && end > 0 && bytes[end - 1] != b'\n' {
            end += 1;
        }
        if end > start {
            chunks.push(&bytes[start..end]);
        }
        start = end;
    }
    chunks
}

fn parse_chunks(
    chunks: &[&[u8]],
    visit: impl Fn([u32; 3]) -> io::Result<()> + Sync,
) -> io::Result<usize> {
    thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|&chunk| {
                let visit = &visit;
                scope.spawn(move || {
                    let mut count = 0;
                    for line in chunk.split_inclusive(|&byte| byte == b'\n') {
                        visit(parse_fields(line)?)?;
                        count += 1;
                    }
                    Ok::<_, io::Error>(count)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|_| io::Error::other("graph worker panicked"))?
            })
            .sum()
    })
}

fn build_graph(bytes: &[u8], workers: usize) -> io::Result<Graph> {
    let invalid = |message| io::Error::new(io::ErrorKind::InvalidData, message);
    if bytes.is_empty() {
        return Err(invalid("missing graph header"));
    }
    let header_end = bytes
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(bytes.len(), |i| i + 1);
    let [n, m] = parse_fields(&bytes[..header_end])?;
    let (n, m) = (n as usize, m as usize);
    let chunks = line_chunks(&bytes[header_end..], workers);
    let degrees: Vec<_> = (0..n).map(|_| AtomicU32::new(0)).collect();
    let incoming: Vec<_> = (0..n.div_ceil(64)).map(|_| AtomicU64::new(0)).collect();
    let count = parse_chunks(&chunks, |[u, v, _]| {
        if u as usize >= n || v as usize >= n {
            return Err(invalid("edge vertex outside graph"));
        }
        degrees[u as usize].fetch_add(1, Ordering::Relaxed);
        incoming[v as usize / 64].fetch_or(1_u64 << (v % 64), Ordering::Relaxed);
        Ok(())
    })?;
    if count != m {
        return Err(invalid("edge count differs from header"));
    }
    let mut offsets = vec![0_u32; n + 1];
    for (i, degree) in degrees.into_iter().enumerate() {
        offsets[i + 1] = offsets[i] + degree.into_inner();
    }
    let cursor: Vec<_> = offsets[..n]
        .iter()
        .map(|&offset| AtomicU32::new(offset))
        .collect();
    let mut edges = vec![(0, 0); m];
    let slots = AtomicPtr::new(edges.as_mut_ptr());
    parse_chunks(&chunks, |[u, v, w]| {
        if u as usize >= n || v as usize >= n {
            return Err(invalid("edge vertex outside graph"));
        }
        let index = cursor[u as usize].fetch_add(1, Ordering::Relaxed) as usize;
        if index >= offsets[u as usize + 1] as usize {
            return Err(invalid("edge count changed"));
        }
        // Each record belongs to exactly one newline-aligned chunk. Atomic
        // cursors assign unique, in-bounds slots. The Vec never moves or reallocates,
        // and no edge is read until every scoped writer has joined. Adjacent slots
        // are disjoint objects. Only edge order within a vertex is nondeterministic.
        unsafe {
            slots.load(Ordering::Relaxed).add(index).write((v, w));
        }
        Ok(())
    })?;
    Ok(Graph {
        offsets,
        edges,
        incoming: incoming.into_iter().map(AtomicU64::into_inner).collect(),
    })
}

fn read_graph(path: &Path, workers: usize) -> io::Result<Graph> {
    let mapping = MappedFile::open(path)?;
    build_graph(mapping.bytes(), workers)
}

fn reorder_graph(graph: Graph, queries: &mut [(usize, usize)], workers: usize) -> Graph {
    if queries.is_empty() {
        return graph;
    }
    let nodes = graph.offsets.len() - 1;
    let mut mapping = vec![u32::MAX; nodes];
    let mut order = Vec::with_capacity(nodes);
    let mut head = 0;
    for root in 0..nodes {
        if mapping[root] != u32::MAX {
            continue;
        }
        mapping[root] = order.len() as u32;
        order.push(root as u32);
        while head < order.len() {
            let node = order[head] as usize;
            head += 1;
            for &(target, _) in
                &graph.edges[graph.offsets[node] as usize..graph.offsets[node + 1] as usize]
            {
                if mapping[target as usize] == u32::MAX {
                    mapping[target as usize] = order.len() as u32;
                    order.push(target);
                }
            }
        }
    }
    let mut offsets = Vec::with_capacity(nodes + 1);
    let mut incoming = vec![0_u64; nodes.div_ceil(64)];
    offsets.push(0);
    for (new, &old) in order.iter().enumerate() {
        let old = old as usize;
        if graph.incoming[old / 64] & (1_u64 << (old % 64)) != 0 {
            incoming[new / 64] |= 1_u64 << (new % 64);
        }
        let degree = graph.offsets[old + 1] - graph.offsets[old];
        offsets.push(offsets.last().unwrap() + degree);
    }
    let mut edges = vec![(0, 0); graph.edges.len()];
    let rows_per_worker = nodes.div_ceil(workers.clamp(1, 12)).max(1);
    thread::scope(|scope| {
        let mut remaining = edges.as_mut_slice();
        for (chunk, old_nodes) in order.chunks(rows_per_worker).enumerate() {
            let first = chunk * rows_per_worker;
            let length = (offsets[first + old_nodes.len()] - offsets[first]) as usize;
            let (destination, rest) = remaining.split_at_mut(length);
            remaining = rest;
            let graph = &graph;
            let mapping = &mapping;
            scope.spawn(move || {
                let mut cursor = 0;
                for &old in old_nodes {
                    let old = old as usize;
                    for &(target, weight) in
                        &graph.edges[graph.offsets[old] as usize..graph.offsets[old + 1] as usize]
                    {
                        destination[cursor] = (mapping[target as usize], weight);
                        cursor += 1;
                    }
                }
            });
        }
    });
    for (source, target) in queries {
        *source = mapping[*source] as usize;
        *target = mapping[*target] as usize;
    }
    Graph {
        offsets,
        edges,
        incoming,
    }
}

/// Distance width used by one Dijkstra run; `MAX` marks unvisited vertices.
trait Distance: Copy + Ord + Into<u64> {
    const MAX: Self;
    const ZERO: Self;
    /// Adds an edge weight, or returns `None` if the sum does not fit below `MAX`.
    fn relax(self, weight: u32) -> Option<Self>;
}

impl Distance for u32 {
    const MAX: Self = u32::MAX;
    const ZERO: Self = 0;
    fn relax(self, weight: u32) -> Option<Self> {
        self.checked_add(weight).filter(|&sum| sum != u32::MAX)
    }
}

impl Distance for u64 {
    const MAX: Self = u64::MAX;
    const ZERO: Self = 0;
    fn relax(self, weight: u32) -> Option<Self> {
        // Paths have at most n - 1 < 2^21 edges, so sums stay far below 2^63.
        Some(self + u64::from(weight))
    }
}

struct Search<D> {
    distances: Vec<D>,
    touched: Vec<u32>,
    heap: BinaryHeap<Reverse<(D, u32)>>,
    overflowed: bool,
}

impl<D: Distance> Search<D> {
    fn new(nodes: usize) -> Self {
        Self {
            distances: vec![D::MAX; nodes],
            touched: Vec::new(),
            heap: BinaryHeap::new(),
            overflowed: false,
        }
    }

    /// Dijkstra that skips relaxations whose sum does not fit `D`. A found
    /// distance is exact: every prefix of a shorter path would also fit. `None`
    /// with `overflowed` set means the answer needs a wider distance type.
    fn run(&mut self, graph: &Graph, source: u32, target: u32) -> Option<u64> {
        self.overflowed = false;
        let found = self.search(graph, source, target);
        for &node in &self.touched {
            self.distances[node as usize] = D::MAX;
        }
        self.touched.clear();
        self.heap.clear();
        found
    }

    fn search(&mut self, graph: &Graph, source: u32, target: u32) -> Option<u64> {
        self.distances[source as usize] = D::ZERO;
        self.touched.push(source);
        self.heap.push(Reverse((D::ZERO, source)));
        while let Some(Reverse((distance, node))) = self.heap.pop() {
            if distance > self.distances[node as usize] {
                continue;
            }
            if node == target {
                return Some(distance.into());
            }
            let range =
                graph.offsets[node as usize] as usize..graph.offsets[node as usize + 1] as usize;
            for &(next, weight) in &graph.edges[range] {
                let Some(candidate) = distance.relax(weight) else {
                    self.overflowed = true;
                    continue;
                };
                let current = &mut self.distances[next as usize];
                if candidate < *current {
                    if *current == D::MAX {
                        self.touched.push(next);
                    }
                    *current = candidate;
                    self.heap.push(Reverse((candidate, next)));
                }
            }
        }
        None
    }
}

/// Per-worker query state: a compact `u32` search, plus a `u64` search that is
/// allocated only for the rare query whose answer may not fit in `u32`.
struct Searcher {
    narrow: Search<u32>,
    wide: Option<Search<u64>>,
}

impl Searcher {
    fn new(nodes: usize) -> Self {
        Self {
            narrow: Search::new(nodes),
            wide: None,
        }
    }

    fn shortest_path(&mut self, graph: &Graph, source: usize, target: usize) -> Option<u64> {
        if source == target {
            return Some(0);
        }
        if graph.offsets[source] == graph.offsets[source + 1]
            || graph.incoming[target / 64] & (1_u64 << (target % 64)) == 0
        {
            return None;
        }
        let (source, target) = (source as u32, target as u32);
        let found = self.narrow.run(graph, source, target);
        if found.is_some() || !self.narrow.overflowed {
            return found;
        }
        let nodes = graph.offsets.len() - 1;
        self.wide
            .get_or_insert_with(|| Search::new(nodes))
            .run(graph, source, target)
    }
}

fn answer_queries(graph: &Graph, queries: &[(usize, usize)], workers: usize) -> Vec<Option<u64>> {
    if queries.is_empty() {
        return Vec::new();
    }
    let workers = workers.clamp(1, 12).min(queries.len());
    if workers == 1 {
        let mut searcher = Searcher::new(graph.offsets.len() - 1);
        return queries
            .iter()
            .map(|&(source, target)| searcher.shortest_path(graph, source, target))
            .collect();
    }
    let next = AtomicUsize::new(0);
    let answers: Vec<_> = queries.iter().map(|_| AtomicU64::new(u64::MAX)).collect();
    thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                let mut searcher = Searcher::new(graph.offsets.len() - 1);
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(source, target)) = queries.get(index) else {
                        break;
                    };
                    let distance = searcher.shortest_path(graph, source, target);
                    answers[index].store(distance.unwrap_or(u64::MAX), Ordering::Relaxed);
                }
            });
        }
    });
    // Scoped joins make all relaxed answer stores visible before collection.
    answers
        .into_iter()
        .map(|answer| match answer.into_inner() {
            u64::MAX => None,
            distance => Some(distance),
        })
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut data = PathBuf::from("/data");
    let mut out = PathBuf::from("/out");
    let mut workers = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(12);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data" => data = args.next().ok_or("missing --data value")?.into(),
            "--out" => out = args.next().ok_or("missing --out value")?.into(),
            "--threads" => {
                workers = args
                    .next()
                    .ok_or("missing --threads value")?
                    .parse::<std::num::NonZeroUsize>()?
                    .get();
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let graph = read_graph(&data.join("graph.txt"), workers)?;
    let mut reader = BufReader::new(File::open(data.join("queries.txt"))?);
    let mut buffer = Vec::with_capacity(32);
    let mut queries = Vec::new();
    while let Some([source, target]) = read_record(&mut reader, &mut buffer)? {
        if source as usize >= graph.offsets.len() - 1 || target as usize >= graph.offsets.len() - 1
        {
            return Err("query vertex outside graph".into());
        }
        queries.push((source as usize, target as usize));
    }
    let graph = reorder_graph(graph, &mut queries, workers);
    fs::create_dir_all(&out)?;
    let mut output = BufWriter::new(File::create(out.join("distances.txt"))?);
    for answer in answer_queries(&graph, &queries, workers) {
        match answer {
            Some(distance) => writeln!(output, "{distance}")?,
            None => writeln!(output, "-1")?,
        }
    }
    output.flush()?;
    Ok(())
}
