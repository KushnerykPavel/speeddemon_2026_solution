use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

struct Graph {
    offsets: Vec<u32>,
    edges: Vec<(u32, u32)>,
}

fn read_graph(path: &Path) -> Result<Graph, Box<dyn Error>> {
    let mut lines = BufReader::new(File::open(path)?).lines();
    let header = lines.next().ok_or("missing graph header")??;
    let fields: Vec<_> = header.split_whitespace().collect();
    let n: usize = fields[0].parse()?;
    let m: usize = fields[1].parse()?;
    let mut list = Vec::with_capacity(m);
    let mut offsets = vec![0_u32; n + 1];
    for line in lines {
        let line = line?;
        let fields: Vec<_> = line.split_whitespace().collect();
        let u: usize = fields[0].parse()?;
        let v: u32 = fields[1].parse()?;
        let w: u32 = fields[2].parse()?;
        offsets[u + 1] += 1;
        list.push((u as u32, v, w));
    }
    if list.len() != m {
        return Err("edge count differs from header".into());
    }
    for i in 0..n {
        offsets[i + 1] += offsets[i];
    }
    let mut cursor = offsets.clone();
    let mut edges = vec![(0, 0); m];
    for (u, v, w) in list {
        edges[cursor[u as usize] as usize] = (v, w);
        cursor[u as usize] += 1;
    }
    Ok(Graph { offsets, edges })
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

fn main() -> Result<(), Box<dyn Error>> {
    let mut data = PathBuf::from("/data");
    let mut out = PathBuf::from("/out");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data" => data = args.next().ok_or("missing --data value")?.into(),
            "--out" => out = args.next().ok_or("missing --out value")?.into(),
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    let graph = read_graph(&data.join("graph.txt"))?;
    let queries = BufReader::new(File::open(data.join("queries.txt"))?)
        .lines()
        .map(|line| {
            let line = line?;
            let fields: Vec<_> = line.split_whitespace().collect();
            Ok((fields[0].parse::<usize>()?, fields[1].parse::<usize>()?))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    fs::create_dir_all(&out)?;
    let mut output = BufWriter::new(File::create(out.join("distances.txt"))?);
    let mut searcher = Searcher::new(graph.offsets.len() - 1);
    for (source, target) in queries {
        match searcher.shortest_path(&graph, source, target) {
            Some(distance) => writeln!(output, "{distance}")?,
            None => writeln!(output, "-1")?,
        }
    }
    output.flush()?;
    Ok(())
}
