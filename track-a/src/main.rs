use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

// The spec fixes every timestamp between 2026-10-30T18 and 2026-11-02T17 (72 hours).
const HOURS: usize = 72;
const DATES: [&str; 4] = ["2026-10-30", "2026-10-31", "2026-11-01", "2026-11-02"];
const LANES: u64 = 0x0101_0101_0101_0101;
const BAD: &str = "malformed log record";

// Eight bytes at `i`; bytes past the end of `data` read as zero.
fn word(data: &[u8], i: usize) -> u64 {
    match data.get(i..i + 8) {
        Some(bytes) => u64::from_le_bytes(bytes.try_into().unwrap()),
        None => {
            let mut tail = [0; 8];
            let rest = data.get(i..).unwrap_or_default();
            tail[..rest.len()].copy_from_slice(rest);
            u64::from_le_bytes(tail)
        }
    }
}

// Offset of the first space in `w`, if any.
fn space_in(w: u64) -> Option<usize> {
    let x = w ^ (0x20 * LANES);
    let found = x.wrapping_sub(LANES) & !x & (0x80 * LANES);
    (found != 0).then(|| found.trailing_zeros() as usize / 8)
}

fn hash_word(hash: u64, w: u64) -> u64 {
    (hash.rotate_left(5) ^ w).wrapping_mul(0x517c_c1b7_2722_0a95)
}

fn digit(data: &[u8], i: usize) -> Option<u8> {
    data.get(i)?.checked_sub(b'0').filter(|d| *d < 10)
}

// Hour of the timestamp at `p`, counted from 2026-10-30T18.
fn hour_index(data: &[u8], p: usize) -> Option<usize> {
    let two = |i| {
        Some(usize::from(
            digit(data, p + i)? * 10 + digit(data, p + i + 1)?,
        ))
    };
    let day_index = match two(5)? {
        10 => two(8)?.checked_sub(30)?,
        11 => two(8)? + 1,
        _ => return None,
    };
    (day_index * 24 + two(11)?)
        .checked_sub(18)
        .filter(|h| *h < HOURS)
}

// One worker's data: an endpoint table (open addressing, full byte comparison)
// and one vector of `error << 63 | endpoint << 32 | latency` keys per hour.
struct State {
    names: Vec<&'static [u8]>,
    hashes: Vec<u64>,
    slots: Vec<u32>,
    hours: Vec<Vec<u64>>,
}

impl State {
    fn new() -> Self {
        State {
            names: Vec::new(),
            hashes: Vec::new(),
            slots: vec![0; 4096],
            hours: vec![Vec::new(); HOURS],
        }
    }

    fn endpoint(&mut self, name: &'static [u8], hash: u64) -> u64 {
        let mask = self.slots.len() - 1;
        let mut i = (hash >> 32) as usize & mask;
        while self.slots[i] != 0 {
            let id = self.slots[i] as usize - 1;
            if self.hashes[id] == hash && self.names[id] == name {
                return id as u64;
            }
            i = (i + 1) & mask;
        }
        self.names.push(name);
        self.hashes.push(hash);
        self.slots[i] = self.names.len() as u32;
        if self.names.len() * 2 > self.slots.len() {
            self.slots = vec![0; self.slots.len() * 2];
            let mask = self.slots.len() - 1;
            for (id, hash) in self.hashes.iter().enumerate() {
                let mut i = (hash >> 32) as usize & mask;
                while self.slots[i] != 0 {
                    i = (i + 1) & mask;
                }
                self.slots[i] = id as u32 + 1;
            }
        }
        self.names.len() as u64 - 1
    }

    // Parses the whole lines `data[pos..end]`: `timestamp method endpoint status latency`.
    fn scan(
        &mut self,
        data: &'static [u8],
        mut pos: usize,
        end: usize,
    ) -> Result<(), &'static str> {
        while pos < end {
            let hour = hour_index(data, pos).ok_or(BAD)?;
            if data.get(pos + 27) != Some(&b' ') {
                return Err(BAD);
            }
            let method = space_in(word(data, pos + 28))
                .filter(|&n| n > 0)
                .ok_or(BAD)?;
            let start = pos + 29 + method;
            let (mut i, mut hash) = (start, 0);
            let length = loop {
                let w = word(data, i);
                if let Some(n) = space_in(w) {
                    hash = hash_word(hash, w & ((1_u64 << (8 * n)) - 1));
                    break i + n - start;
                }
                hash = hash_word(hash, w);
                i += 8;
                if i >= end {
                    return Err(BAD);
                }
            };
            let status = start + length + 1;
            let ok = (0..3).all(|k| digit(data, status + k).is_some());
            if length == 0 || !ok || data.get(status + 3) != Some(&b' ') {
                return Err(BAD);
            }
            let (mut j, mut latency) = (status + 4, 0_u64);
            while let Some(d) = digit(data, j) {
                latency = latency * 10 + u64::from(d);
                j += 1;
                if latency > u64::from(u32::MAX) {
                    return Err(BAD);
                }
            }
            if j == status + 4 {
                return Err(BAD);
            }
            if data.get(j) == Some(&b'\r') {
                j += 1;
            }
            if j > end || j < data.len() && data[j] != b'\n' {
                return Err(BAD);
            }
            pos = j + 1;
            let id = self.endpoint(&data[start..start + length], hash ^ length as u64);
            let error = u64::from(data[status] == b'5');
            self.hours[hour].push(error << 63 | id << 32 | latency);
        }
        Ok(())
    }
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

// Splits `bytes` into up to `pieces` ranges that start and end after a newline.
fn line_chunks(bytes: &[u8], pieces: usize) -> Vec<(usize, usize)> {
    let (mut chunks, mut start) = (Vec::new(), 0);
    for left in (1..=pieces.max(1)).rev() {
        let target = start + (bytes.len() - start) / left;
        let end = bytes[target..]
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(bytes.len(), |i| target + i + 1);
        if end > start {
            chunks.push((start, end));
        }
        start = end;
    }
    chunks
}

// Runs `job` on `workers` scoped threads and returns their results.
fn run<T: Send>(workers: usize, job: impl Fn() -> T + Sync) -> Vec<T> {
    thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.max(1)).map(|_| scope.spawn(&job)).collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    })
}

fn summarize(data: &'static [u8], workers: usize) -> Result<Vec<Vec<u8>>, &'static str> {
    let chunks = line_chunks(data, workers * 8);
    let next = AtomicUsize::new(0);
    let states = run(workers, || {
        let mut state = State::new();
        while let Some(&(start, end)) = chunks.get(next.fetch_add(1, Ordering::Relaxed)) {
            state.scan(data, start, end)?;
        }
        Ok(state)
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;

    let mut endpoints: Vec<&[u8]> = states
        .iter()
        .flat_map(|s| s.names.iter().copied())
        .collect();
    endpoints.sort_unstable();
    endpoints.dedup();
    // remap[state][local endpoint ID] is the endpoint's rank in byte order.
    let remap: Vec<Vec<u64>> = states
        .iter()
        .map(|s| {
            s.names
                .iter()
                .map(|n| endpoints.binary_search(n).unwrap() as u64)
                .collect()
        })
        .collect();

    let next = AtomicUsize::new(0);
    let mut texts: Vec<(usize, Vec<u8>)> = run(workers, || {
        let (mut done, mut keys, mut errors) = (Vec::new(), Vec::new(), vec![0; endpoints.len()]);
        loop {
            let hour = next.fetch_add(1, Ordering::Relaxed);
            if hour >= HOURS {
                return done;
            }
            keys.clear();
            errors.fill(0);
            for (state, ranks) in states.iter().zip(&remap) {
                for &key in &state.hours[hour] {
                    let rank = ranks[(key >> 32 & 0x7fff_ffff) as usize];
                    errors[rank as usize] += key >> 63;
                    keys.push(rank << 32 | key & 0xffff_ffff);
                }
            }
            keys.sort_unstable();
            let (mut text, mut i) = (Vec::new(), 0);
            while i < keys.len() {
                let rank = keys[i] >> 32;
                let count = keys[i..].partition_point(|&key| key >> 32 == rank);
                let at = |percent: usize| keys[i + (percent * count).div_ceil(100) - 1] as u32;
                let (date, h) = (DATES[(hour + 18) / 24], (hour + 18) % 24);
                let errs: u64 = errors[rank as usize];
                write!(text, "{date}T{h:02}:00:00Z ").unwrap();
                text.extend_from_slice(endpoints[rank as usize]);
                let ppm = errs * 1_000_000 / count as u64;
                writeln!(
                    text,
                    " {count} {} {} {} {errs} {ppm}",
                    at(50),
                    at(95),
                    at(99)
                )
                .unwrap();
                i += count;
            }
            done.push((hour, text));
        }
    })
    .concat();
    texts.sort_unstable_by_key(|&(hour, _)| hour);
    // The caller exits right away, so the worker state is not freed.
    std::mem::forget(states);
    Ok(texts.into_iter().map(|(_, text)| text).collect())
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
    let texts = summarize(map_file(data.join("requests.log"))?, workers)?;
    fs::create_dir_all(&out)?;
    let mut output = File::create(out.join("summary.txt"))?;
    for text in &texts {
        output.write_all(text)?;
    }
    // Skip freeing the hour texts and unmapping the input.
    std::process::exit(0);
}
