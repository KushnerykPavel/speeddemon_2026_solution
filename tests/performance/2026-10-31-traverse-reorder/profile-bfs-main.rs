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

fn reorder_graph(graph: Graph, queries: &mut [(usize, usize)]) -> Graph {
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
    let mut edges = Vec::with_capacity(graph.edges.len());
    let mut incoming = vec![0_u64; nodes.div_ceil(64)];
    offsets.push(0);
    for (new, &old) in order.iter().enumerate() {
        let old = old as usize;
        if graph.incoming[old / 64] & (1_u64 << (old % 64)) != 0 {
            incoming[new / 64] |= 1_u64 << (new % 64);
        }
        for &(target, weight) in
            &graph.edges[graph.offsets[old] as usize..graph.offsets[old + 1] as usize]
        {
            edges.push((mapping[target as usize], weight));
        }
        offsets.push(edges.len() as u32);
    }
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

struct Searcher {
    distances: Vec<u64>,
    touched: Vec<u32>,
    heap: BinaryHeap<Reverse<(u64, u32)>>,
}

impl Searcher {
    fn new(nodes: usize) -> Self {
        Self {
            distances: vec![u64::MAX; nodes],
            touched: Vec::new(),
            heap: BinaryHeap::new(),
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
        let found = self.search(graph, source as u32, target as u32);
        for &node in &self.touched {
            self.distances[node as usize] = u64::MAX;
        }
        self.touched.clear();
        self.heap.clear();
        found
    }

    fn search(&mut self, graph: &Graph, source: u32, target: u32) -> Option<u64> {
        self.distances[source as usize] = 0;
        self.touched.push(source);
        self.heap.push(Reverse((0, source)));
        while let Some(Reverse((distance, node))) = self.heap.pop() {
            if distance > self.distances[node as usize] {
                continue;
            }
            if node == target {
                return Some(distance);
            }
            let range =
                graph.offsets[node as usize] as usize..graph.offsets[node as usize + 1] as usize;
            for &(next, weight) in &graph.edges[range] {
                let candidate = distance + u64::from(weight);
                let current = &mut self.distances[next as usize];
                if candidate < *current {
                    if *current == u64::MAX {
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
    let phase = std::time::Instant::now();
    let graph = read_graph(&data.join("graph.txt"), workers)?;
    let graph_s = phase.elapsed().as_secs_f64();
    let phase = std::time::Instant::now();
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
    let query_parse_s = phase.elapsed().as_secs_f64();
    let phase = std::time::Instant::now();
    let graph = reorder_graph(graph, &mut queries);
    let reorder_s = phase.elapsed().as_secs_f64();
    fs::create_dir_all(&out)?;
    let mut output = BufWriter::new(File::create(out.join("distances.txt"))?);
    let phase = std::time::Instant::now();
    let answers = answer_queries(&graph, &queries, workers);
    let search_s = phase.elapsed().as_secs_f64();
    let phase = std::time::Instant::now();
    for answer in answers {
        match answer {
            Some(distance) => writeln!(output, "{distance}")?,
            None => writeln!(output, "-1")?,
        }
    }
    output.flush()?;
    let output_s = phase.elapsed().as_secs_f64();
    eprintln!("PHASES={{\"graph_s\":{},\"query_parse_s\":{},\"reorder_s\":{},\"search_s\":{},\"output_s\":{}}}", graph_s, query_parse_s, reorder_s, search_s, output_s);
    Ok(())
}
