use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

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
    last_hour: Option<([u8; 13], usize)>,
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
    let hour_id = match groups.last_hour {
        Some((hour, id)) if hour == record.hour => id,
        _ => {
            let id = *groups.hours.entry(record.hour).or_insert_with(|| {
                groups.rows.push(Vec::new());
                groups.rows.len() - 1
            });
            groups.last_hour = Some((record.hour, id));
            id
        }
    };
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

// Records are parsed from one chunk of whole lines; the final line may lack a newline.
fn read_chunk(chunk: &[u8]) -> Result<Groups, &'static str> {
    let mut groups = Groups::default();
    let mut lines = chunk.split(|&byte| byte == b'\n').peekable();
    while let Some(line) = lines.next() {
        if line.is_empty() && lines.peek().is_none() {
            break;
        }
        add_record(&mut groups, line.strip_suffix(b"\r").unwrap_or(line))?;
    }
    Ok(groups)
}

// The competition target and local development hosts use 64-bit Unix off_t.
unsafe extern "C" {
    fn mmap(
        address: *mut u8,
        length: usize,
        prot: i32,
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

// 4 KB pages instead of transparent huge pages, and large freed blocks go back
// to the kernel. Failures only cost memory, so results are ignored.
fn tune_memory() {
    #[cfg(target_os = "linux")]
    unsafe {
        prctl(41, 1, 0, 0, 0);
        mallopt(-3, 256 << 10);
    }
}

// Read-only private mapping for the rest of the run; the data mount is read-only.
fn map_file(path: PathBuf) -> std::io::Result<&'static [u8]> {
    let file = File::open(path)?;
    let length = usize::try_from(file.metadata()?.len()).map_err(std::io::Error::other)?;
    if length == 0 {
        return Ok(&[]);
    }
    let address = unsafe { mmap(std::ptr::null_mut(), length, 1, 2, file.as_raw_fd(), 0) };
    if address as isize == -1 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { std::slice::from_raw_parts(address, length) })
}

// Splits `bytes` into up to `workers` chunks of whole lines.
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

// Runs `task` over 0..count on `workers` scoped threads claiming indices from a counter.
fn parallel<T: Send>(count: usize, workers: usize, task: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, T)> = thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.clamp(1, count.max(1)))
            .map(|_| {
                scope.spawn(|| {
                    let mut local = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= count {
                            return local;
                        }
                        local.push((index, task(index)));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect()
    });
    done.sort_unstable_by_key(|&(index, _)| index);
    done.into_iter().map(|(_, value)| value).collect()
}

fn percentile(values: &[u32], percent: usize) -> u32 {
    values[(percent * values.len()).div_ceil(100) - 1]
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut data = PathBuf::from("/data");
    let mut out = PathBuf::from("/out");
    let mut workers = thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(12);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data" => data = args.next().ok_or("missing --data value")?.into(),
            "--out" => out = args.next().ok_or("missing --out value")?.into(),
            "--threads" => workers = args.next().ok_or("missing --threads value")?.parse()?,
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }

    tune_memory();
    let bytes = map_file(data.join("requests.log"))?;
    let chunks = line_chunks(bytes, workers);
    let parts = parallel(chunks.len(), workers, |i| read_chunk(chunks[i]))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    let mut hours: Vec<[u8; 13]> = parts.iter().flat_map(|p| p.hours.keys().copied()).collect();
    hours.sort_unstable();
    hours.dedup();
    let mut endpoints: Vec<&str> = parts
        .iter()
        .flat_map(|p| p.endpoints.keys())
        .map(String::as_str)
        .collect();
    endpoints.sort_unstable();
    endpoints.dedup();
    // remap[part][rank] is the part's own endpoint ID for the rank-th sorted endpoint.
    let remap: Vec<Vec<Option<usize>>> = parts
        .iter()
        .map(|p| {
            endpoints
                .iter()
                .map(|e| p.endpoints.get(*e).copied())
                .collect()
        })
        .collect();

    let texts = parallel(hours.len(), workers, |h| {
        let hour = std::str::from_utf8(&hours[h]).unwrap();
        let rows: Vec<_> = parts
            .iter()
            .map(|p| p.hours.get(&hours[h]).map(|&id| &p.rows[id]))
            .collect();
        let (mut text, mut latencies) = (String::new(), Vec::new());
        for (rank, endpoint) in endpoints.iter().enumerate() {
            latencies.clear();
            let mut errors = 0;
            for (row, ids) in rows.iter().zip(&remap) {
                let group = row.zip(ids[rank]).and_then(|(row, id)| row.get(id));
                if let Some(group) = group {
                    latencies.extend_from_slice(&group.latencies);
                    errors += group.errors;
                }
            }
            let count = latencies.len();
            if count == 0 {
                continue;
            }
            latencies.sort_unstable();
            let ppm = errors * 1_000_000 / count as u64;
            writeln!(
                text,
                "{hour}:00:00Z {endpoint} {count} {} {} {} {errors} {ppm}",
                percentile(&latencies, 50),
                percentile(&latencies, 95),
                percentile(&latencies, 99),
            )
            .unwrap();
        }
        text
    });
    fs::create_dir_all(&out)?;
    let mut output = File::create(out.join("summary.txt"))?;
    for text in &texts {
        output.write_all(text.as_bytes())?;
    }
    // Skip freeing the groups and unmapping the input.
    std::process::exit(0);
}
