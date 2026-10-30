use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

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

    let mut groups: HashMap<[u8; 13], HashMap<String, Group>> = HashMap::new();
    let mut reader = BufReader::new(File::open(data.join("requests.log"))?);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        let mut fields = line.split(' ');
        let (Some(timestamp), Some(_method), Some(endpoint), Some(status), Some(latency), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            return Err("expected five log fields".into());
        };
        // The spec guarantees fixed-width, valid UTC timestamps. Their first
        // thirteen bytes identify the calendar hour, including month changes.
        let hour = timestamp.get(..13).ok_or("invalid timestamp")?;
        let hour: [u8; 13] = hour.as_bytes().try_into()?;
        let latency = latency.parse()?;
        let status: u16 = status.parse()?;
        let errors = u64::from((500..=599).contains(&status));
        let endpoints = groups.entry(hour).or_default();
        if let Some(group) = endpoints.get_mut(endpoint) {
            group.latencies.push(latency);
            group.errors += errors;
        } else {
            endpoints.insert(
                endpoint.to_owned(),
                Group {
                    latencies: vec![latency],
                    errors,
                },
            );
        }
    }

    let mut groups: Vec<_> = groups
        .into_iter()
        .flat_map(|(hour, endpoints)| {
            endpoints
                .into_iter()
                .map(move |(endpoint, group)| ((hour, endpoint), group))
        })
        .collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    fs::create_dir_all(&out)?;
    let mut output = BufWriter::new(File::create(out.join("summary.txt"))?);
    for ((hour, endpoint), mut group) in groups {
        let hour = std::str::from_utf8(&hour)?;
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
