use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, BufWriter, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
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
    Ok(Some(fields))
}

fn read_graph(path: &Path) -> Result<Graph, Box<dyn Error>> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut buffer = Vec::with_capacity(64);
    let [n, m] = read_record(&mut reader, &mut buffer)?.ok_or("missing graph header")?;
    let (n, m) = (n as usize, m as usize);
    let graph_stage = std::time::Instant::now();
    let mut count = 0;
    let mut offsets = vec![0_u32; n + 1];
    let mut incoming = vec![0_u64; n.div_ceil(64)];
    while let Some([u, v, _]) = read_record(&mut reader, &mut buffer)? {
        if u as usize >= n || v as usize >= n {
            return Err("edge vertex outside graph".into());
        }
        offsets[u as usize + 1] += 1;
        incoming[v as usize / 64] |= 1_u64 << (v % 64);
        count += 1;
    }
    if count != m {
        return Err("edge count differs from header".into());
    }
    let graph_first_pass_s = graph_stage.elapsed().as_secs_f64();
    let graph_stage = std::time::Instant::now();
    for i in 0..n {
        offsets[i + 1] += offsets[i];
    }
    let mut cursor = offsets.clone();
    let mut edges = vec![(0, 0); m];
    let graph_prefix_allocate_s = graph_stage.elapsed().as_secs_f64();
    let graph_stage = std::time::Instant::now();
    reader.rewind()?;
    if read_record(&mut reader, &mut buffer)? != Some([n as u32, m as u32]) {
        return Err("graph header changed between passes".into());
    }
    while let Some([u, v, w]) = read_record(&mut reader, &mut buffer)? {
        if u as usize >= n || v as usize >= n || cursor[u as usize] >= offsets[u as usize + 1] {
            return Err("graph edges changed between passes".into());
        }
        edges[cursor[u as usize] as usize] = (v, w);
        cursor[u as usize] += 1;
    }
    if cursor[..n] != offsets[1..] {
        return Err("edge count changed between passes".into());
    }
    let graph_scatter_s = graph_stage.elapsed().as_secs_f64();
    eprintln!("GRAPH_PHASES={{\"first_pass_s\":{},\"prefix_allocate_s\":{},\"scatter_s\":{}}}", graph_first_pass_s, graph_prefix_allocate_s, graph_scatter_s);
    Ok(Graph {
        offsets,
        edges,
        incoming,
    })
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
    let graph = read_graph(&data.join("graph.txt"))?;
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
    eprintln!("PHASES={{\"graph_s\":{},\"query_parse_s\":{},\"search_s\":{},\"output_s\":{}}}", graph_s, query_parse_s, search_s, output_s);
    Ok(())
}
