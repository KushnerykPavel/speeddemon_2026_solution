use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

#[derive(Default)]
struct Group {
    latencies: Vec<u32>,
    errors: u64,
}

fn percentile(values: &[u32], percent: usize) -> u32 {
    values[(percent * values.len()).div_ceil(100) - 1]
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

    let mut groups: HashMap<(String, String), Group> = HashMap::new();
    for line in BufReader::new(File::open(data.join("requests.log"))?).lines() {
        let line = line?;
        let fields: Vec<_> = line.split(' ').collect();
        if fields.len() != 5 {
            return Err("expected five log fields".into());
        }
        // The spec guarantees fixed-width, valid UTC timestamps. Their first
        // thirteen bytes identify the calendar hour, including month changes.
        let hour = fields[0].get(..13).ok_or("invalid timestamp")?;
        let group = groups
            .entry((hour.to_owned(), fields[2].to_owned()))
            .or_default();
        group.latencies.push(fields[4].parse()?);
        let status: u16 = fields[3].parse()?;
        if (500..=599).contains(&status) {
            group.errors += 1;
        }
    }

    let mut groups: Vec<_> = groups.into_iter().collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    fs::create_dir_all(&out)?;
    let mut output = BufWriter::new(File::create(out.join("summary.txt"))?);
    for ((hour, endpoint), mut group) in groups {
        group.latencies.sort();
        let count = group.latencies.len();
        let ppm = group.errors * 1_000_000 / count as u64;
        writeln!(
            output,
            "{hour}:00:00Z {endpoint} {count} {} {} {} {} {ppm}",
            percentile(&group.latencies, 50),
            percentile(&group.latencies, 95),
            percentile(&group.latencies, 99),
            group.errors
        )?;
    }
    output.flush()?;
    Ok(())
}
