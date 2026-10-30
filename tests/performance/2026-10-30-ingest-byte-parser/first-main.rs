use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;

struct Group {
    latencies: Vec<u32>,
    errors: u64,
}

type Groups = HashMap<[u8; 13], HashMap<String, Group>>;

#[derive(Debug)]
struct Record<'a> {
    hour: [u8; 13],
    endpoint: &'a str,
    latency: u32,
    error: bool,
}

fn parse_decimal(bytes: &[u8]) -> Result<u32, &'static str> {
    if bytes.is_empty() {
        return Err("empty integer");
    }
    let mut value = 0_u32;
    for &byte in bytes {
        if !byte.is_ascii_digit() {
            return Err("invalid integer");
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u32::from(byte - b'0')))
            .ok_or("integer overflow")?;
    }
    Ok(value)
}

// The caller removes the line ending; all returned borrows stay within this row.
fn parse_record(line: &[u8]) -> Result<Record<'_>, &'static str> {
    let mut spaces = [0; 4];
    let mut count = 0;
    for (index, &byte) in line.iter().enumerate() {
        if byte == b' ' {
            if count == spaces.len() {
                return Err("expected five log fields");
            }
            spaces[count] = index;
            count += 1;
        }
    }
    if count != spaces.len() {
        return Err("expected five log fields");
    }
    let [a, b, c, d] = spaces;
    if a == 0 || b == a + 1 || c == b + 1 || d == c + 1 || d + 1 == line.len() {
        return Err("empty log field");
    }
    // Valid UTC timestamps have a fixed width; the first thirteen bytes name the hour.
    let hour: [u8; 13] = line[..a]
        .get(..13)
        .ok_or("invalid timestamp")?
        .try_into()
        .map_err(|_| "invalid timestamp")?;
    if !hour.is_ascii() {
        return Err("invalid timestamp");
    }
    let endpoint = std::str::from_utf8(&line[b + 1..c]).map_err(|_| "invalid endpoint")?;
    let status = parse_decimal(&line[c + 1..d])?;
    if status > u32::from(u16::MAX) {
        return Err("status overflow");
    }
    Ok(Record {
        hour,
        endpoint,
        latency: parse_decimal(&line[d + 1..])?,
        error: (500..=599).contains(&status),
    })
}

fn add_record(groups: &mut Groups, line: &[u8]) -> Result<(), &'static str> {
    let record = parse_record(line)?;
    let errors = u64::from(record.error);
    let endpoints = groups.entry(record.hour).or_default();
    if let Some(group) = endpoints.get_mut(record.endpoint) {
        group.latencies.push(record.latency);
        group.errors += errors;
    } else {
        endpoints.insert(
            record.endpoint.to_owned(),
            Group {
                latencies: vec![record.latency],
                errors,
            },
        );
    }
    Ok(())
}

fn read_groups(reader: &mut impl BufRead) -> Result<Groups, Box<dyn Error>> {
    let mut groups = Groups::new();
    let mut partial = Vec::new();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            if !partial.is_empty() {
                add_record(&mut groups, &partial)?;
            }
            return Ok(groups);
        }
        let mut start = 0;
        while let Some(offset) = buffer[start..].iter().position(|&byte| byte == b'\n') {
            let end = start + offset;
            let line = if partial.is_empty() {
                &buffer[start..end]
            } else {
                partial.extend_from_slice(&buffer[start..end]);
                &partial
            };
            add_record(&mut groups, line.strip_suffix(b"\r").unwrap_or(line))?;
            partial.clear();
            start = end + 1;
        }
        partial.extend_from_slice(&buffer[start..]);
        let consumed = buffer.len();
        reader.consume(consumed);
    }
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

    let mut reader = BufReader::with_capacity(64 * 1024, File::open(data.join("requests.log"))?);
    let groups = read_groups(&mut reader)?;

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
        group.latencies.sort_unstable();
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
