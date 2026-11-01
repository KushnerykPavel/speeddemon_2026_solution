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

#[derive(Default)]
struct Groups {
    endpoints: HashMap<String, usize>,
    hours: HashMap<[u8; 13], usize>,
    rows: Vec<Vec<Group>>,
}

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
    let mut fields = line.split(|&byte| byte == b' ');
    let (Some(timestamp), Some(method), Some(endpoint), Some(status), Some(latency), None) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Err("expected five log fields");
    };
    if method.is_empty() || endpoint.is_empty() {
        return Err("empty log field");
    }
    // Valid UTC timestamps have a fixed width; the first thirteen bytes name the hour.
    let hour: [u8; 13] = timestamp
        .get(..13)
        .ok_or("invalid timestamp")?
        .try_into()
        .map_err(|_| "invalid timestamp")?;
    if !hour.is_ascii() {
        return Err("invalid timestamp");
    }
    let endpoint = std::str::from_utf8(endpoint).map_err(|_| "invalid endpoint")?;
    let status = parse_decimal(status)?;
    if status > u32::from(u16::MAX) {
        return Err("status overflow");
    }
    Ok(Record {
        hour,
        endpoint,
        latency: parse_decimal(latency)?,
        error: (500..=599).contains(&status),
    })
}

fn add_record(groups: &mut Groups, line: &[u8]) -> Result<(), &'static str> {
    let record = parse_record(line)?;
    let endpoint_id = match groups.endpoints.get(record.endpoint) {
        Some(&id) => id,
        None => {
            let id = groups.endpoints.len();
            groups.endpoints.insert(record.endpoint.to_owned(), id);
            id
        }
    };
    let hour_id = *groups.hours.entry(record.hour).or_insert_with(|| {
        groups.rows.push(Vec::new());
        groups.rows.len() - 1
    });
    let row = &mut groups.rows[hour_id];
    // Rows grow independently: discovering an endpoint never changes existing IDs.
    if row.len() <= endpoint_id {
        row.resize_with(endpoint_id + 1, Group::default);
    }
    let group = &mut row[endpoint_id];
    group.latencies.push(record.latency);
    group.errors += u64::from(record.error);
    Ok(())
}

fn read_groups(reader: &mut impl BufRead) -> Result<Groups, Box<dyn Error>> {
    let mut groups = Groups::default();
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

    let mut hours: Vec<_> = groups.hours.into_iter().collect();
    hours.sort_unstable_by_key(|&(hour, _)| hour);
    let mut endpoints: Vec<_> = groups.endpoints.into_iter().collect();
    endpoints.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    let mut rows = groups.rows;
    fs::create_dir_all(&out)?;
    let mut output = BufWriter::new(File::create(out.join("summary.txt"))?);
    for (hour, hour_id) in hours {
        let hour = std::str::from_utf8(&hour)?;
        let row = &mut rows[hour_id];
        for (endpoint, endpoint_id) in &endpoints {
            let Some(group) = row.get_mut(*endpoint_id) else {
                continue;
            };
            let count = group.latencies.len();
            if count == 0 {
                continue;
            }
            group.latencies.sort_unstable();
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
    }
    output.flush()?;
    Ok(())
}
