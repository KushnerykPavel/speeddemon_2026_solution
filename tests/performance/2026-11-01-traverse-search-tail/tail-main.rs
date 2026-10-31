use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, AtomicUsize};
use std::thread;

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Hints the CPU to start loading `value`'s cache line; no effect elsewhere.
fn prefetch<T>(value: &T) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
        _mm_prefetch::<_MM_HINT_T0>((value as *const T).cast());
    }
    #[cfg(not(target_arch = "x86_64"))]
    let _ = value;
}

/// Exact weights of edges whose packed weight is the escape code, keyed by edge
/// index; filled by parallel builders and sorted when the graph is finished.
type Heavy = Mutex<Vec<(u32, u32)>>;

/// Packs an edge into one `u32`: the other endpoint in the low `head_bits`, the
/// weight above it. A weight that does not fit is stored as the all-ones escape
/// code with its exact value in the graph's `heavy` list.
#[derive(Clone, Copy)]
struct Packing {
    head_bits: u32,
}

impl Packing {
    fn for_nodes(nodes: usize) -> io::Result<Self> {
        let head_bits = (usize::BITS - nodes.saturating_sub(1).leading_zeros()).max(1);
        if head_bits >= 32 {
            return Err(invalid("too many vertices"));
        }
        Ok(Self { head_bits })
    }

    fn escape(self) -> u32 {
        u32::MAX >> self.head_bits
    }

    /// Returns `(head, weight code)`.
    fn unpack(self, packed: u32) -> (u32, u32) {
        let mask = (1 << self.head_bits) - 1;
        (packed & mask, packed >> self.head_bits)
    }

    fn pack(self, index: usize, head: u32, weight: u32, heavy: &Heavy) -> u32 {
        if weight >= self.escape() {
            heavy.lock().unwrap().push((index as u32, weight));
        }
        head | weight.min(self.escape()) << self.head_bits
    }
}

/// Compressed sparse rows: the packed edges of vertex `v` are
/// `edges[offsets[v]..offsets[v + 1]]`; `row(v)` yields `(other endpoint, weight)`.
struct Graph {
    offsets: Vec<u32>,
    edges: Vec<u32>,
    heavy: Vec<(u32, u32)>,
    packing: Packing,
}

impl Graph {
    fn new(offsets: Vec<u32>, edges: Vec<u32>, heavy: Heavy, packing: Packing) -> Self {
        let mut heavy = heavy.into_inner().unwrap();
        heavy.sort_unstable();
        Self {
            offsets,
            edges,
            heavy,
            packing,
        }
    }

    /// Starts loading vertex `node`'s row bounds and first edges into cache.
    fn prefetch_row(&self, node: usize) {
        prefetch(&self.offsets[node]);
        if let Some(edge) = self.edges.get(self.offsets[node] as usize) {
            prefetch(edge);
        }
    }

    fn row(&self, node: usize) -> impl Iterator<Item = (u32, u32)> + '_ {
        let start = self.offsets[node] as usize;
        let edges = &self.edges[start..self.offsets[node + 1] as usize];
        edges.iter().enumerate().map(move |(offset, &packed)| {
            let (head, code) = self.packing.unpack(packed);
            if code != self.packing.escape() {
                return (head, code);
            }
            let index = (start + offset) as u32;
            let at = self.heavy.partition_point(|&(heavy, _)| heavy < index);
            (head, self.heavy[at].1)
        })
    }
}

/// Parses one line of exactly `N` whitespace-separated decimal `u32` fields.
fn parse_fields<const N: usize>(line: &[u8]) -> io::Result<[u32; N]> {
    let number = |field: &[u8]| {
        let digits = field.iter().all(u8::is_ascii_digit);
        digits
            .then(|| std::str::from_utf8(field).ok()?.parse().ok())
            .flatten()
    };
    let mut fields = line
        .split(u8::is_ascii_whitespace)
        .filter(|field| !field.is_empty());
    let mut values = [0; N];
    for value in &mut values {
        *value = fields
            .next()
            .and_then(number)
            .ok_or_else(|| invalid("expected uint32 fields"))?;
    }
    match fields.next() {
        Some(_) => Err(invalid("expected uint32 fields")),
        None => Ok(values),
    }
}

// The competition target and local development hosts use 64-bit Unix off_t.
unsafe extern "C" {
    fn mmap(
        address: *mut u8,
        length: usize,
        protection: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut u8;
}

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    fn mallopt(parameter: i32, value: i32) -> i32;
}

/// Keeps memory that is no longer used out of the cgroup peak. Asks Linux for
/// 4 KB pages instead of transparent huge pages (buffers here are touched
/// sparsely; with 2 MB pages one touched byte commits 2 MB), and fixes glibc's
/// mmap threshold at 256 KB so large freed blocks go back to the kernel instead
/// of the threshold growing to the largest block freed. Failures only cost
/// memory, so results are ignored.
fn tune_memory() {
    #[cfg(target_os = "linux")]
    unsafe {
        const PR_SET_THP_DISABLE: i32 = 41;
        const M_MMAP_THRESHOLD: i32 = -3;
        prctl(PR_SET_THP_DISABLE, 1, 0, 0, 0);
        mallopt(M_MMAP_THRESHOLD, 256 << 10);
    }
}

/// Maps `path` read-only (PROT_READ, MAP_PRIVATE) for the rest of the run. The
/// competition data mount is read-only, so the file cannot change underneath.
fn map_file(path: &Path) -> io::Result<&'static [u8]> {
    let file = File::open(path)?;
    let length = usize::try_from(file.metadata()?.len()).map_err(io::Error::other)?;
    if length == 0 {
        return Ok(&[]);
    }
    let address = unsafe { mmap(std::ptr::null_mut(), length, 1, 2, file.as_raw_fd(), 0) };
    if address as isize == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { std::slice::from_raw_parts(address, length) })
}

/// Splits `bytes` into up to `workers` chunks of whole lines.
fn line_chunks(bytes: &[u8], workers: usize) -> Vec<&[u8]> {
    let (mut chunks, mut rest) = (Vec::new(), bytes);
    for left in (1..=workers.clamp(1, 12)).rev() {
        let start = rest.len() / left;
        let end = rest[start..]
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(rest.len(), |i| start + i + 1);
        let (chunk, tail) = rest.split_at(end);
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        rest = tail;
    }
    chunks
}

/// Reads a field of one to seven ASCII digits ending in `separator`, using one
/// 8-byte load: bytes with bit 4 clear end the number, every byte before must be
/// a digit, and pairs, quads and octets of digits are combined with three
/// multiplies. Returns the value and the bytes used, separator included.
fn fast_field(bytes: &[u8], separator: u8) -> Option<(u32, usize)> {
    const LANES: u64 = 0x0101_0101_0101_0101;
    const BIT4: u64 = 0x10 * LANES;
    const ZEROS: u64 = 0x30 * LANES;
    const HIGH: u64 = 0xF0 * LANES;
    const SIX: u64 = 6 * LANES;
    let word = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
    let length = (!word & BIT4).trailing_zeros() as usize / 8;
    if length == 0 || length == 8 || bytes[length] != separator {
        return None;
    }
    let keep = (1_u64 << (8 * length)) - 1;
    let digits = (word & keep).wrapping_sub(ZEROS & keep);
    // Every kept byte must be 0x30..=0x39: high nibble 3, low nibble below 10.
    if (word & keep & HIGH) != (ZEROS & keep) || ((digits + SIX) & keep & HIGH) != 0 {
        return None;
    }
    let mut value = digits << (64 - 8 * length);
    value = (value * 10 + (value >> 8)) & 0x00FF_00FF_00FF_00FF;
    value = (value * 100 + (value >> 16)) & 0x0000_FFFF_0000_FFFF;
    value = (value * 10_000 + (value >> 32)) & 0xFFFF_FFFF;
    Some((value as u32, length + 1))
}

/// Parses a plain `u v w\n` record without a per-digit loop, or returns `None`
/// so the caller falls back to the strict line parser (CRLF, tabs, repeated
/// spaces, numbers over seven digits, the chunk tail, invalid bytes).
fn fast_record(bytes: &[u8]) -> Option<([u32; 3], usize)> {
    let (source, first) = fast_field(bytes, b' ')?;
    let (target, second) = fast_field(&bytes[first..], b' ')?;
    let (weight, third) = fast_field(&bytes[first + second..], b'\n')?;
    Some(([source, target, weight], first + second + third))
}

/// Parses every chunk's records on its own scoped worker. Each worker owns a
/// state from `init` that `visit` updates; returns the record count and states.
fn parse_chunks<S: Send>(
    chunks: &[&[u8]],
    init: impl Fn() -> S + Sync,
    visit: impl Fn(&mut S, [u32; 3]) -> io::Result<()> + Sync,
) -> io::Result<(usize, Vec<S>)> {
    let parse = |mut rest: &[u8]| {
        let (mut count, mut state) = (0, init());
        while !rest.is_empty() {
            let (record, used) = match fast_record(rest) {
                Some(parsed) => parsed,
                None => {
                    let end = rest
                        .iter()
                        .position(|&byte| byte == b'\n')
                        .map_or(rest.len(), |i| i + 1);
                    (parse_fields(&rest[..end])?, end)
                }
            };
            visit(&mut state, record)?;
            (count, rest) = (count + 1, &rest[used..]);
        }
        Ok::<_, io::Error>((count, state))
    };
    thread::scope(|scope| {
        let handles: Vec<_> = chunks
            .iter()
            .map(|&chunk| scope.spawn(move || parse(chunk)))
            .collect();
        let (mut total, mut states) = (0, Vec::new());
        for handle in handles {
            let (count, state) = handle.join().unwrap()?;
            total += count;
            states.push(state);
        }
        Ok((total, states))
    })
}

/// Edges parsed by one chunk worker, bucketed by the part that owns their source.
/// Per part: `(source, packed target and weight)` records, plus the exact
/// weights of escaped records in record order.
type Buckets = Vec<(Vec<(u32, u32)>, Vec<u32>)>;

/// Parses the graph in one pass. Workers bucket each edge by its source's part
/// (a power-of-two block of IDs, about eight per worker); then each part counts
/// and places its own edges into its own disjoint slices of the CSR, a region
/// small enough to stay in cache, with no atomics. Within a row, edges keep
/// chunk order and then line order.
fn build_graph(bytes: &[u8], workers: usize) -> io::Result<Graph> {
    if bytes.is_empty() {
        return Err(invalid("missing graph header"));
    }
    let header_end = bytes
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(bytes.len(), |i| i + 1);
    let [n, m] = parse_fields(&bytes[..header_end])?.map(|value| value as usize);
    let packing = Packing::for_nodes(n)?;
    let workers = workers.clamp(1, 12);
    let part_bits = n
        .div_ceil(8 * workers)
        .next_power_of_two()
        .trailing_zeros()
        .max(10);
    let parts = n.div_ceil(1 << part_bits);
    let chunks = line_chunks(&bytes[header_end..], workers);
    let (count, buckets) = parse_chunks(
        &chunks,
        || vec![(Vec::new(), Vec::new()); parts],
        |buckets: &mut Buckets, [u, v, w]| {
            if u as usize >= n || v as usize >= n {
                return Err(invalid("edge vertex outside graph"));
            }
            let (records, weights) = &mut buckets[(u >> part_bits) as usize];
            if w >= packing.escape() {
                weights.push(w);
            }
            records.push((u, v | w.min(packing.escape()) << packing.head_bits));
            Ok(())
        },
    )?;
    if count != m {
        return Err(invalid("edge count differs from header"));
    }
    let part_sizes: Vec<usize> = (0..parts)
        .map(|part| buckets.iter().map(|bucket| bucket[part].0.len()).sum())
        .collect();
    let mut offsets = vec![0_u32; n + 1];
    let mut edges = vec![0; m];
    let heavy = Mutex::new(Vec::new());
    thread::scope(|scope| {
        let (mut offset_rest, mut edge_rest) = (&mut offsets[..n], edges.as_mut_slice());
        let mut base = 0;
        let parts_per_worker = parts.div_ceil(workers).max(1);
        for first in (0..parts).step_by(parts_per_worker) {
            let last = (first + parts_per_worker).min(parts);
            let nodes_here = (last << part_bits).min(n) - (first << part_bits);
            let edges_here: usize = part_sizes[first..last].iter().sum();
            let (mut row_starts, rest) = offset_rest.split_at_mut(nodes_here);
            offset_rest = rest;
            let (mut slots, rest) = edge_rest.split_at_mut(edges_here);
            edge_rest = rest;
            let (buckets, part_sizes, heavy, worker_base) = (&buckets, &part_sizes, &heavy, base);
            scope.spawn(move || {
                let mut base = worker_base;
                for part in first..last {
                    let start = part << part_bits;
                    let (part_starts, rest) =
                        row_starts.split_at_mut((n - start).min(1 << part_bits));
                    row_starts = rest;
                    let (part_slots, rest) = slots.split_at_mut(part_sizes[part]);
                    slots = rest;
                    let records = || {
                        buckets.iter().flat_map(|bucket| {
                            let (records, weights) = &bucket[part];
                            let mut weights = weights.iter();
                            records.iter().map(move |&(u, packed)| {
                                let (v, code) = packing.unpack(packed);
                                let heavy = code == packing.escape();
                                (
                                    u,
                                    v,
                                    if heavy {
                                        *weights.next().unwrap()
                                    } else {
                                        code
                                    },
                                )
                            })
                        })
                    };
                    for (u, _, _) in records() {
                        part_starts[u as usize - start] += 1;
                    }
                    let mut total = 0;
                    for row in part_starts.iter_mut() {
                        (*row, total) = (total, total + *row);
                    }
                    let mut cursor = part_starts.to_vec();
                    for (u, v, w) in records() {
                        let next = &mut cursor[u as usize - start];
                        let index = (base + *next) as usize;
                        part_slots[*next as usize] = packing.pack(index, v, w, heavy);
                        *next += 1;
                    }
                    for row in part_starts.iter_mut() {
                        *row += base;
                    }
                    base += part_sizes[part] as u32;
                }
            });
            base += edges_here as u32;
        }
    });
    offsets[n] = m as u32;
    Ok(Graph::new(offsets, edges, heavy, packing))
}

/// Renumbers vertices so searches touch nearby memory. Every worker grows
/// breadth-first regions from unclaimed roots in its own slice of old IDs and
/// claims each vertex with a compare-and-swap on `worker << bits | position + 1`
/// (0 = free), noting each vertex's degree as it is visited. The regions, one
/// after another, are the new order. A parallel pass over equal pieces of that
/// order then copies the rows from the old graph into the new CSR, translating
/// heads through the claims, so only the two packed graphs, the claims and the
/// orders are ever held at once.
fn reorder_graph(graph: Graph, queries: &mut [(usize, usize)], workers: usize) -> Graph {
    let nodes = graph.offsets.len() - 1;
    // Provisional IDs need worker bits above the position bits within a u32.
    if queries.is_empty() || nodes >= 1 << 27 {
        return graph;
    }
    let workers = workers.clamp(1, 12);
    let bits = usize::BITS - nodes.leading_zeros();
    let claims: Vec<_> = (0..nodes).map(|_| AtomicU32::new(0)).collect();
    let claim = |node: usize, id: usize| {
        let slot = &claims[node];
        slot.load(Relaxed) == 0
            && slot
                .compare_exchange(0, id as u32, Relaxed, Relaxed)
                .is_ok()
    };
    let regions: Vec<(Vec<u32>, Vec<u32>)> = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|worker| {
                let (graph, claim) = (&graph, &claim);
                scope.spawn(move || {
                    let (mut order, mut degrees) = (Vec::new(), Vec::new());
                    for root in nodes * worker / workers..nodes * (worker + 1) / workers {
                        if !claim(root, worker << bits | (order.len() + 1)) {
                            continue;
                        }
                        order.push(root as u32);
                        while degrees.len() < order.len() {
                            // Breadth-first reads are random: ask for rows ahead.
                            if let Some(&ahead) = order.get(degrees.len() + 8) {
                                prefetch(&graph.offsets[ahead as usize]);
                            }
                            if let Some(&ahead) = order.get(degrees.len() + 4) {
                                graph.prefetch_row(ahead as usize);
                            }
                            let node = order[degrees.len()] as usize;
                            degrees.push(graph.offsets[node + 1] - graph.offsets[node]);
                            for (target, _) in graph.row(node) {
                                if claim(target as usize, worker << bits | (order.len() + 1)) {
                                    order.push(target);
                                }
                            }
                        }
                    }
                    (order, degrees)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    let mut node_base = vec![0];
    let mut new_offsets = Vec::with_capacity(nodes + 1);
    new_offsets.push(0_u32);
    for (order, degrees) in &regions {
        node_base.push(node_base.last().unwrap() + order.len() as u32);
        for degree in degrees {
            new_offsets.push(new_offsets.last().unwrap() + degree);
        }
    }
    let orders: Vec<_> = regions.into_iter().map(|(order, _)| order).collect();
    let mask = (1_u32 << bits) - 1;
    let final_id = |node: usize| {
        let id = claims[node].load(Relaxed);
        node_base[(id >> bits) as usize] + (id & mask) - 1
    };
    let mut new_edges = vec![0; new_offsets[nodes] as usize];
    let heavy = Mutex::new(Vec::new());
    thread::scope(|scope| {
        // Regions can differ widely in size, so work is dealt out in equal pieces.
        let mut jobs: Vec<Vec<_>> = (0..workers).map(|_| Vec::new()).collect();
        let (mut rest, mut first) = (new_edges.as_mut_slice(), 0);
        for (index, order) in orders
            .iter()
            .flat_map(|order| order.chunks(nodes.div_ceil(workers * 8)))
            .enumerate()
        {
            let base = new_offsets[first] as usize;
            first += order.len();
            let (slots, tail) = rest.split_at_mut(new_offsets[first] as usize - base);
            rest = tail;
            jobs[index % workers].push((order, base, slots));
        }
        for job in jobs {
            let (graph, final_id, heavy) = (&graph, &final_id, &heavy);
            scope.spawn(move || {
                for (order, base, slots) in job {
                    let mut written = 0;
                    for (position, &old) in order.iter().enumerate() {
                        if let Some(&ahead) = order.get(position + 4) {
                            graph.prefetch_row(ahead as usize);
                        }
                        for (target, weight) in graph.row(old as usize) {
                            let head = final_id(target as usize);
                            slots[written] =
                                graph.packing.pack(base + written, head, weight, heavy);
                            written += 1;
                        }
                    }
                }
            });
        }
    });
    let packing = graph.packing;
    drop(graph);
    for (source, target) in queries {
        (*source, *target) = (final_id(*source) as usize, final_id(*target) as usize);
    }
    Graph::new(new_offsets, new_edges, heavy, packing)
}

/// Reverse graph: row `v` lists `(u, w)` for every edge `u -> v` of `graph`.
/// Workers count in-degrees over disjoint source rows, then scatter each edge
/// into a slot claimed from its target's atomic cursor.
fn transpose(graph: &Graph, workers: usize) -> Graph {
    let nodes = graph.offsets.len() - 1;
    let workers = workers.clamp(1, 12);
    let for_each_edge = |visit: &(dyn Fn(u32, u32, u32) + Sync)| {
        thread::scope(|scope| {
            for worker in 0..workers {
                scope.spawn(move || {
                    for source in nodes * worker / workers..nodes * (worker + 1) / workers {
                        for (target, weight) in graph.row(source) {
                            visit(source as u32, target, weight);
                        }
                    }
                });
            }
        });
    };
    let degrees: Vec<_> = (0..nodes).map(|_| AtomicU32::new(0)).collect();
    for_each_edge(&|_, target, _| {
        degrees[target as usize].fetch_add(1, Relaxed);
    });
    let mut offsets = vec![0_u32; nodes + 1];
    for (i, degree) in degrees.into_iter().enumerate() {
        offsets[i + 1] = offsets[i] + degree.into_inner();
    }
    let cursor: Vec<_> = offsets[..nodes]
        .iter()
        .map(|&offset| AtomicU32::new(offset))
        .collect();
    let mut edges = vec![0; graph.edges.len()];
    let slots = AtomicPtr::new(edges.as_mut_ptr());
    let heavy = Mutex::new(Vec::new());
    for_each_edge(&|source, target, weight| {
        let index = cursor[target as usize].fetch_add(1, Relaxed) as usize;
        let packed = graph.packing.pack(index, source, weight, &heavy);
        // Every edge is visited once and the counted cursors hand out unique,
        // in-bounds slots; the Vec does not move and is read only after the joins.
        unsafe { slots.load(Relaxed).add(index).write(packed) }
    });
    Graph::new(offsets, edges, heavy, graph.packing)
}

/// Distance width used by one Dijkstra run; `MAX` marks unvisited vertices.
trait Distance: Copy + Ord + Default + Into<u64> {
    const MAX: Self;
    /// Heap key ordered like `(distance, node)`.
    type Key: Copy + Ord;
    /// Adds an edge weight, or returns `None` if the sum does not fit below `MAX`.
    fn relax(self, weight: u32) -> Option<Self>;
    fn key(self, node: u32) -> Self::Key;
    fn unkey(key: Self::Key) -> (Self, u32);
}

impl Distance for u32 {
    const MAX: Self = u32::MAX;
    /// Distance in the high half, node in the low half: one comparison per sift.
    type Key = u64;
    fn relax(self, weight: u32) -> Option<Self> {
        self.checked_add(weight).filter(|&sum| sum != u32::MAX)
    }
    fn key(self, node: u32) -> u64 {
        u64::from(self) << 32 | u64::from(node)
    }
    fn unkey(key: u64) -> (Self, u32) {
        ((key >> 32) as u32, key as u32)
    }
}

impl Distance for u64 {
    const MAX: Self = u64::MAX;
    type Key = (u64, u32);
    fn relax(self, weight: u32) -> Option<Self> {
        // Paths have fewer than 2^31 edges below 2^32 each, so sums stay below 2^63.
        Some(self + u64::from(weight))
    }
    fn key(self, node: u32) -> (u64, u32) {
        (self, node)
    }
    fn unkey(key: (u64, u32)) -> (Self, u32) {
        key
    }
}

/// Vertices per distance page (4 KB of `u32`). Renumbered searches are local,
/// so a query touches few pages.
const PAGE: usize = 1 << 10;
const NO_PAGE: u32 = u32::MAX;

/// One direction of a bidirectional search. Distances live in pages that are
/// assigned on first write and returned after each query, so memory follows
/// the largest search instead of the vertex count.
struct Side<D: Distance> {
    page_of_block: Vec<u32>,
    pages: Vec<D>,
    used_blocks: Vec<u32>,
    heap: BinaryHeap<Reverse<D::Key>>,
}

impl<D: Distance> Side<D> {
    fn new(nodes: usize) -> Self {
        let page_of_block = vec![NO_PAGE; nodes.div_ceil(PAGE)];
        Self {
            page_of_block,
            pages: Vec::new(),
            used_blocks: Vec::new(),
            heap: BinaryHeap::new(),
        }
    }

    fn distance(&self, node: u32) -> D {
        match self.page_of_block[node as usize / PAGE] {
            NO_PAGE => D::MAX,
            page => self.pages[page as usize * PAGE + node as usize % PAGE],
        }
    }

    fn reach(&mut self, node: u32, distance: D) {
        let block = node as usize / PAGE;
        if self.page_of_block[block] == NO_PAGE {
            self.page_of_block[block] = self.used_blocks.len() as u32;
            self.used_blocks.push(block as u32);
            self.pages
                .resize(self.pages.len().max(self.used_blocks.len() * PAGE), D::MAX);
        }
        self.pages[self.page_of_block[block] as usize * PAGE + node as usize % PAGE] = distance;
        self.heap.push(Reverse(distance.key(node)));
    }

    /// Smallest queued key; a stale key is still a valid lower bound.
    fn top(&self) -> Option<u64> {
        self.heap.peek().map(|&Reverse(key)| D::unkey(key).0.into())
    }

    /// Settles the next vertex and relaxes its edges in `graph`. Returns the
    /// shortest path length through any edge that reaches a vertex `other` has
    /// already labelled. Sums that do not fit `D` are skipped and flagged.
    fn settle_next(&mut self, graph: &Graph, other: &Self, overflowed: &mut bool) -> u64 {
        let Some(Reverse(key)) = self.heap.pop() else {
            return u64::MAX;
        };
        let (distance, node) = D::unkey(key);
        if distance > self.distance(node) {
            return u64::MAX;
        }
        if let Some(&Reverse(next)) = self.heap.peek() {
            graph.prefetch_row(D::unkey(next).1 as usize);
        }
        let mut meeting = u64::MAX;
        for (next, weight) in graph.row(node as usize) {
            let Some(candidate) = distance.relax(weight) else {
                *overflowed = true;
                continue;
            };
            let across = other.distance(next);
            if across != D::MAX {
                meeting = meeting.min(candidate.into() + across.into());
            }
            if candidate < self.distance(next) {
                self.reach(next, candidate);
            }
        }
        meeting
    }

    /// Clears the pages this query used and makes them available again.
    fn reset(&mut self) {
        for (page, &block) in self.used_blocks.iter().enumerate() {
            self.page_of_block[block as usize] = NO_PAGE;
            self.pages[page * PAGE..(page + 1) * PAGE].fill(D::MAX);
        }
        self.used_blocks.clear();
        self.heap.clear();
    }
}

/// Bidirectional Dijkstra: forward from the source over `forward`, backward
/// from the target over the reverse graph. Each step grows the side with the
/// smaller queue, so a side that is about to run out (an island, a dead end)
/// finishes first instead of letting the other side flood the graph.
struct Search<D: Distance> {
    forward: Side<D>,
    backward: Side<D>,
    overflowed: bool,
}

/// `u64` search shared by all workers, allocated on the first query whose answer
/// may not fit in `u32`. One lock-protected instance keeps memory at one array.
type WideSearch = Mutex<Option<Search<u64>>>;

impl<D: Distance> Search<D> {
    fn new(nodes: usize) -> Self {
        Self {
            forward: Side::new(nodes),
            backward: Side::new(nodes),
            overflowed: false,
        }
    }

    /// Stops when the two queue minima sum to at least the best path seen, or a
    /// side runs out (then every path has been seen). A path shorter than
    /// `D::MAX` is exact even if larger sums were skipped: all its prefixes and
    /// suffixes fit. Otherwise `overflowed` tells the caller to widen.
    fn run(&mut self, [forward, backward]: [&Graph; 2], source: u32, target: u32) -> Option<u64> {
        self.overflowed = false;
        self.forward.reach(source, D::default());
        self.backward.reach(target, D::default());
        let mut best = u64::MAX;
        while let (Some(ahead), Some(behind)) = (self.forward.top(), self.backward.top()) {
            if ahead + behind >= best {
                break;
            }
            let meeting = if self.forward.heap.len() <= self.backward.heap.len() {
                self.forward
                    .settle_next(forward, &self.backward, &mut self.overflowed)
            } else {
                self.backward
                    .settle_next(backward, &self.forward, &mut self.overflowed)
            };
            best = best.min(meeting);
        }
        self.forward.reset();
        self.backward.reset();
        (best != u64::MAX).then_some(best)
    }
}

impl Search<u32> {
    /// Answers one query with this compact search, repeating it in the shared
    /// `u64` search only when a sum overflowed and no shorter `u32` path exists.
    fn shortest_path(
        &mut self,
        wide: &WideSearch,
        graphs: [&Graph; 2],
        source: usize,
        target: usize,
    ) -> Option<u64> {
        let [forward, backward] = graphs;
        if source == target {
            return Some(0);
        }
        if forward.row(source).next().is_none() || backward.row(target).next().is_none() {
            return None;
        }
        let (source, target) = (source as u32, target as u32);
        let found = self.run(graphs, source, target);
        if !self.overflowed || found.is_some_and(|distance| distance < u32::MAX.into()) {
            return found;
        }
        let nodes = forward.offsets.len() - 1;
        wide.lock()
            .unwrap()
            .get_or_insert_with(|| Search::new(nodes))
            .run(graphs, source, target)
    }
}

fn answer_queries(graph: &Graph, queries: &[(usize, usize)], workers: usize) -> Vec<Option<u64>> {
    let clock = std::time::Instant::now();
    let graphs = [graph, &transpose(graph, workers)];
    let transpose_s = clock.elapsed().as_secs_f64();
    let (wide, next) = (Mutex::new(None), AtomicUsize::new(0));
    let answers: Vec<_> = queries.iter().map(|_| AtomicU64::new(u64::MAX)).collect();
    let log = Mutex::new(Vec::new());
    let ends = Mutex::new(Vec::new());
    let start = std::time::Instant::now();
    thread::scope(|scope| {
        for worker in 0..workers.clamp(1, 12).min(queries.len()) {
            let (log, ends, start) = (&log, &ends, &start);
            let (wide, next, answers) = (&wide, &next, &answers);
            scope.spawn(move || {
                let mut search = Search::new(graph.offsets.len() - 1);
                let mut mine = Vec::new();
                loop {
                    let index = next.fetch_add(1, Relaxed);
                    let Some(&(source, target)) = queries.get(index) else {
                        break;
                    };
                    let begin = start.elapsed().as_secs_f64();
                    let distance = search.shortest_path(wide, graphs, source, target);
                    let end = start.elapsed().as_secs_f64();
                    mine.push((index, worker, begin, end, distance.is_some()));
                    answers[index].store(distance.unwrap_or(u64::MAX), Relaxed);
                }
                ends.lock().unwrap().push((worker, start.elapsed().as_secs_f64()));
                log.lock().unwrap().extend(mine);
            });
        }
    });
    let mut text = String::new();
    for (index, worker, begin, end, found) in log.into_inner().unwrap() {
        text += &format!("[{index},{worker},{begin},{end},{}]", found as u8);
        text += ",";
    }
    text.pop();
    let ends: Vec<String> = ends.into_inner().unwrap().iter().map(|(w, t)| format!("[{w},{t}]")).collect();
    eprintln!("TAIL={{\"transpose_s\":{transpose_s},\"ends\":[{}],\"queries\":[{text}]}}", ends.join(","));
    // Scoped joins make all relaxed answer stores visible before collection.
    answers
        .into_iter()
        .map(|answer| Some(answer.into_inner()).filter(|&distance| distance != u64::MAX))
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    tune_memory();
    let mut data = PathBuf::from("/data");
    let mut out = PathBuf::from("/out");
    let mut workers = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(12);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("missing {arg} value"));
        match arg.as_str() {
            "--data" => data = value()?.into(),
            "--out" => out = value()?.into(),
            "--threads" => workers = value()?.parse::<std::num::NonZeroUsize>()?.get(),
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let graph = build_graph(map_file(&data.join("graph.txt"))?, workers)?;
    let nodes = graph.offsets.len() - 1;
    let mut queries = Vec::new();
    for line in map_file(&data.join("queries.txt"))?.split_inclusive(|&byte| byte == b'\n') {
        let [source, target] = parse_fields(line)?.map(|vertex| vertex as usize);
        if source >= nodes || target >= nodes {
            return Err("query vertex outside graph".into());
        }
        queries.push((source, target));
    }
    let graph = reorder_graph(graph, &mut queries, workers);
    fs::create_dir_all(&out)?;
    let mut output = BufWriter::new(File::create(out.join("distances.txt"))?);
    for answer in answer_queries(&graph, &queries, workers) {
        // Distances stay below 2^63 (see `relax`), so they fit an i64.
        writeln!(output, "{}", answer.map_or(-1, |distance| distance as i64))?;
    }
    output.flush()?;
    Ok(())
}
