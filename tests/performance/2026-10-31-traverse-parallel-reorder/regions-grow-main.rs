use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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

/// One worker's share of the renumbering: the old IDs it claimed in visiting
/// order, each claimed vertex's edge end, and its edges with provisional heads.
struct Region {
    order: Vec<u32>,
    ends: Vec<u32>,
    edges: Vec<(u32, u32)>,
}

/// Renumbers vertices so searches touch nearby memory. Every worker grows
/// breadth-first regions from unclaimed roots in its own slice of old IDs and
/// claims each vertex with a compare-and-swap on `worker << bits | position + 1`
/// (0 = free). Workers write their edges with those provisional heads while
/// visiting, so the old graph is read once; a parallel pass then concatenates
/// the regions into the new CSR, translating provisional IDs to final ones.
fn reorder_graph(graph: Graph, queries: &mut [(usize, usize)], workers: usize) -> Graph {
    let nodes = graph.offsets.len() - 1;
    // Provisional IDs need worker bits above the position bits within a u32.
    if queries.is_empty() || nodes >= 1 << 27 {
        return graph;
    }
    let workers = workers.clamp(1, 12);
    let bits = usize::BITS - nodes.leading_zeros();
    let claims: Vec<_> = (0..nodes).map(|_| AtomicU32::new(0)).collect();
    let Graph {
        offsets,
        edges,
        incoming,
    } = graph;
    let regions: Vec<Region> = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let (offsets, edges, claims) = (&offsets, &edges, &claims);
                scope.spawn(move || {
                    let provisional = |position: usize| (worker << bits | (position + 1)) as u32;
                    let mut region = Region {
                        order: Vec::new(),
                        ends: Vec::new(),
                        edges: Vec::new(),
                    };
                    for root in nodes * worker / workers..nodes * (worker + 1) / workers {
                        let claim = provisional(region.order.len());
                        if claims[root].load(Ordering::Relaxed) != 0
                            || claims[root]
                                .compare_exchange(0, claim, Ordering::Relaxed, Ordering::Relaxed)
                                .is_err()
                        {
                            continue;
                        }
                        let mut head = region.order.len();
                        region.order.push(root as u32);
                        while head < region.order.len() {
                            let node = region.order[head] as usize;
                            head += 1;
                            for &(target, weight) in
                                &edges[offsets[node] as usize..offsets[node + 1] as usize]
                            {
                                let slot = &claims[target as usize];
                                let mut id = slot.load(Ordering::Relaxed);
                                if id == 0 {
                                    let claim = provisional(region.order.len());
                                    id = match slot.compare_exchange(
                                        0,
                                        claim,
                                        Ordering::Relaxed,
                                        Ordering::Relaxed,
                                    ) {
                                        Ok(_) => {
                                            region.order.push(target);
                                            claim
                                        }
                                        Err(owner) => owner,
                                    };
                                }
                                region.edges.push((id, weight));
                            }
                            region.ends.push(region.edges.len() as u32);
                        }
                    }
                    region
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("reorder worker panicked"))
            .collect()
    });
    drop((offsets, edges));
    let mut node_base = Vec::with_capacity(workers);
    let mut edge_base = Vec::with_capacity(workers);
    let (mut node_total, mut edge_total) = (0_u32, 0_u32);
    for region in &regions {
        node_base.push(node_total);
        edge_base.push(edge_total);
        node_total += region.order.len() as u32;
        edge_total += region.edges.len() as u32;
    }
    let mask = (1_u32 << bits) - 1;
    let final_id = |id: u32| node_base[(id >> bits) as usize] + (id & mask) - 1;
    let mut new_offsets = vec![0_u32; nodes + 1];
    let mut new_edges = vec![(0_u32, 0_u32); edge_total as usize];
    thread::scope(|scope| {
        let mut offset_rest = &mut new_offsets[1..];
        let mut edge_rest = new_edges.as_mut_slice();
        for (region, &base) in regions.iter().zip(&edge_base) {
            let (offset_slots, rest) = offset_rest.split_at_mut(region.order.len());
            offset_rest = rest;
            let (edge_slots, rest) = edge_rest.split_at_mut(region.edges.len());
            edge_rest = rest;
            let final_id = &final_id;
            scope.spawn(move || {
                for (slot, &end) in offset_slots.iter_mut().zip(&region.ends) {
                    *slot = base + end;
                }
                for (slot, &(id, weight)) in edge_slots.iter_mut().zip(&region.edges) {
                    *slot = (final_id(id), weight);
                }
            });
        }
    });
    let mut new_incoming = vec![0_u64; nodes.div_ceil(64)];
    for (region, &base) in regions.iter().zip(&node_base) {
        for (position, &old) in region.order.iter().enumerate() {
            let (old, new) = (old as usize, base as usize + position);
            if incoming[old / 64] & (1_u64 << (old % 64)) != 0 {
                new_incoming[new / 64] |= 1_u64 << (new % 64);
            }
        }
    }
    for (source, target) in queries {
        *source = final_id(claims[*source].load(Ordering::Relaxed)) as usize;
        *target = final_id(claims[*target].load(Ordering::Relaxed)) as usize;
    }
    Graph {
        offsets: new_offsets,
        edges: new_edges,
        incoming: new_incoming,
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

/// `u64` search shared by all workers, allocated on the first query whose answer
/// may not fit in `u32`. One lock-protected instance keeps memory at one array.
type WideSearch = Mutex<Option<Search<u64>>>;

/// Per-worker query state: a compact `u32` search plus the shared fallback.
struct Searcher<'a> {
    narrow: Search<u32>,
    wide: &'a WideSearch,
}

impl<'a> Searcher<'a> {
    fn new(nodes: usize, wide: &'a WideSearch) -> Self {
        Self {
            narrow: Search::new(nodes),
            wide,
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
        let mut wide = self
            .wide
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        wide.get_or_insert_with(|| Search::new(nodes))
            .run(graph, source, target)
    }
}

fn answer_queries(graph: &Graph, queries: &[(usize, usize)], workers: usize) -> Vec<Option<u64>> {
    if queries.is_empty() {
        return Vec::new();
    }
    let workers = workers.clamp(1, 12).min(queries.len());
    let wide = Mutex::new(None);
    if workers == 1 {
        let mut searcher = Searcher::new(graph.offsets.len() - 1, &wide);
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
                let mut searcher = Searcher::new(graph.offsets.len() - 1, &wide);
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
