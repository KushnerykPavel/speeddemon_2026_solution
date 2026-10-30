use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

type Graph = Vec<Vec<(usize, u32)>>;

fn read_graph(path: &Path) -> Result<Graph, Box<dyn Error>> {
    let mut lines = BufReader::new(File::open(path)?).lines();
    let header = lines.next().ok_or("missing graph header")??;
    let fields: Vec<_> = header.split_whitespace().collect();
    let n: usize = fields[0].parse()?;
    let m: usize = fields[1].parse()?;
    let mut graph = vec![Vec::new(); n];
    let mut edges = 0;
    for line in lines {
        let line = line?;
        let fields: Vec<_> = line.split_whitespace().collect();
        let u: usize = fields[0].parse()?;
        let v: usize = fields[1].parse()?;
        let w: u32 = fields[2].parse()?;
        graph[u].push((v, w));
        edges += 1;
    }
    if edges != m {
        return Err("edge count differs from header".into());
    }
    Ok(graph)
}

fn shortest_path(graph: &Graph, source: usize, target: usize) -> Option<u64> {
    let mut distances = HashMap::from([(source, 0_u64)]);
    let mut heap = BinaryHeap::from([Reverse((0_u64, source))]);
    while let Some(Reverse((distance, node))) = heap.pop() {
        if distance > distances[&node] {
            continue;
        }
        if node == target {
            return Some(distance);
        }
        for &(next, weight) in &graph[node] {
            let candidate = distance + u64::from(weight);
            if distances
                .get(&next)
                .is_none_or(|&current| candidate < current)
            {
                distances.insert(next, candidate);
                heap.push(Reverse((candidate, next)));
            }
        }
    }
    None
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
    for (source, target) in queries {
        match shortest_path(&graph, source, target) {
            Some(distance) => writeln!(output, "{distance}")?,
            None => writeln!(output, "-1")?,
        }
    }
    output.flush()?;
    Ok(())
}
