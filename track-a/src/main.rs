use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;
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

// Hour of the timestamp at `p`, counted from 2026-10-30T18. One load covers
// `MM-DDTHH` of `YYYY-MM-DDTHH:...`.
fn hour_index(data: &[u8], p: usize) -> Option<usize> {
    let w = word(data, p + 5);
    let two = |at: u32| ((w >> at & 15) * 10 + (w >> (at + 8) & 15)) as usize;
    if w >> 16 & 0xff != u64::from(b'-') || w >> 40 & 0xff != u64::from(b'T') {
        return None;
    }
    let day_index = match two(0) {
        10 => two(24).checked_sub(30)?,
        11 => two(24) + 1,
        _ => return None,
    };
    (day_index * 24 + two(48))
        .checked_sub(18)
        .filter(|h| *h < HOURS)
}

// Decimal number at `i` and its length in digits (at most ten, value below 2^32).
fn number(data: &[u8], i: usize) -> Option<(u64, usize)> {
    let w = word(data, i);
    let length = ((!w & (0x10 * LANES)).trailing_zeros() / 8) as usize;
    if length < 8 {
        let mask = (1_u64 << (8 * length)) - 1;
        let t = (w ^ (0x30 * LANES)) & mask;
        if length == 0 || t & (0xf0 * LANES) != 0 || (t + 6 * LANES) & (0xf0 * LANES) != 0 {
            return None;
        }
        let t = (t << (8 * (8 - length))).wrapping_mul(2561) >> 8;
        let t = (t & 0x00ff_00ff_00ff_00ff).wrapping_mul(6_553_601) >> 16;
        let t = (t & 0x0000_ffff_0000_ffff).wrapping_mul(42_949_672_960_001) >> 32;
        return Some((t, length));
    }
    let mut value = 0_u64;
    let mut length = 0;
    while let Some(d) = digit(data, i + length) {
        value = value * 10 + u64::from(d);
        length += 1;
        if value > u64::from(u32::MAX) {
            return None;
        }
    }
    (length > 0).then_some((value, length))
}

// One worker's data: an endpoint table (open addressing, full byte comparison)
// and `error << 63 | endpoint << 32 | latency` keys. `open` collects the current
// chunk per hour; `flush` moves each hour's keys, trimmed to size, into `hours`.
struct State {
    names: Vec<&'static [u8]>,
    hashes: Vec<u64>,
    slots: Vec<u32>,
    open: Vec<Vec<u64>>,
    hours: Vec<Vec<Vec<u64>>>,
}

impl State {
    fn new() -> Self {
        State {
            names: Vec::new(),
            hashes: Vec::new(),
            slots: vec![0; 4096],
            open: vec![Vec::new(); HOURS],
            hours: vec![Vec::new(); HOURS],
        }
    }

    fn flush(&mut self) {
        for (open, hour) in self.open.iter_mut().zip(&mut self.hours) {
            if !open.is_empty() {
                open.shrink_to_fit();
                hour.push(std::mem::take(open));
            }
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
            let (latency, digits) = number(data, status + 4).ok_or(BAD)?;
            let mut j = status + 4 + digits;
            if data.get(j) == Some(&b'\r') {
                j += 1;
            }
            if j > end || j < data.len() && data[j] != b'\n' {
                return Err(BAD);
            }
            pos = j + 1;
            let id = self.endpoint(&data[start..start + length], hash ^ length as u64);
            let error = u64::from(data[status] == b'5');
            self.open[hour].push(error << 63 | id << 32 | latency);
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

fn itoa(mut value: u64, buffer: &mut [u8; 20]) -> &[u8] {
    let mut at = buffer.len();
    loop {
        at -= 1;
        buffer[at] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            return &buffer[at..];
        }
    }
}

fn summarize(
    data: &'static [u8],
    workers: usize,
    out: &mut impl Write,
) -> Result<(), Box<dyn Error>> {
    let chunks = line_chunks(data, workers * 8);
    let next = AtomicUsize::new(0);
    let states = run(workers, || {
        let mut state = State::new();
        while let Some(&(start, end)) = chunks.get(next.fetch_add(1, Ordering::Relaxed)) {
            state.scan(data, start, end)?;
            state.flush();
        }
        Ok::<State, &'static str>(state)
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;

    // Global endpoint table, built from the workers' stored hashes; ranks follow byte order.
    let mut global = State::new();
    let local_ids: Vec<Vec<u64>> = states
        .iter()
        .map(|s| {
            s.names
                .iter()
                .zip(&s.hashes)
                .map(|(&n, &h)| global.endpoint(n, h))
                .collect()
        })
        .collect();
    let mut order: Vec<usize> = (0..global.names.len()).collect();
    order.sort_unstable_by_key(|&id| global.names[id]);
    let endpoints: Vec<&[u8]> = order.iter().map(|&id| global.names[id]).collect();
    let mut rank_of = vec![0; order.len()];
    for (rank, &id) in order.iter().enumerate() {
        rank_of[id] = rank as u64;
    }
    // remap[state][local endpoint ID] is the endpoint's rank in byte order.
    let remap: Vec<Vec<u64>> = local_ids
        .iter()
        .map(|ids| ids.iter().map(|&id| rank_of[id as usize]).collect())
        .collect();

    // Workers format whole hours; this thread writes them in order as they complete.
    let next = AtomicUsize::new(0);
    let (sender, receiver) = mpsc::channel();
    thread::scope(|scope| {
        for _ in 0..workers.max(1) {
            let (states, remap, endpoints, next, sender) =
                (&states, &remap, &endpoints, &next, sender.clone());
            scope.spawn(move || {
                let ranks = endpoints.len();
                let (mut errors, mut start) = (vec![0; ranks], vec![0; ranks + 1]);
                let (mut cursor, mut latencies) = (vec![0; ranks], Vec::new());
                loop {
                    let hour = next.fetch_add(1, Ordering::Relaxed);
                    if hour >= HOURS {
                        return;
                    }
                    let keys = || {
                        states.iter().zip(remap.iter()).flat_map(|(s, r)| {
                            s.hours[hour]
                                .iter()
                                .flatten()
                                .map(move |&k| (r[(k >> 32 & 0x7fff_ffff) as usize] as usize, k))
                        })
                    };
                    errors.fill(0);
                    start.fill(0);
                    for (rank, key) in keys() {
                        errors[rank] += key >> 63;
                        start[rank + 1] += 1;
                    }
                    for rank in 0..ranks {
                        start[rank + 1] += start[rank];
                    }
                    cursor.copy_from_slice(&start[..ranks]);
                    latencies.resize(start[ranks], 0);
                    for (rank, key) in keys() {
                        latencies[cursor[rank]] = key as u32;
                        cursor[rank] += 1;
                    }
                    let mut text = format!(
                        "{}T{:02}:00:00Z ",
                        DATES[(hour + 18) / 24],
                        (hour + 18) % 24
                    )
                    .into_bytes();
                    let prefix = text.len();
                    for (rank, endpoint) in endpoints.iter().enumerate() {
                        let group = &mut latencies[start[rank]..start[rank + 1]];
                        if group.is_empty() {
                            continue;
                        }
                        let count = group.len();
                        let mut ends = [50, 95, 99].map(|p| (p * count).div_ceil(100) - 1);
                        let mut values = [0; 3];
                        let mut high = count;
                        for (value, index) in values.iter_mut().zip(ends.iter_mut()).rev() {
                            if *index < high {
                                group[..high].select_nth_unstable(*index);
                                high = *index;
                            }
                            *value = group[*index];
                        }
                        text.extend_from_slice(endpoint);
                        let errs = errors[rank];
                        let fields = [
                            count as u64,
                            values[0].into(),
                            values[1].into(),
                            values[2].into(),
                            errs,
                            errs * 1_000_000 / count as u64,
                        ];
                        for field in fields {
                            text.push(b' ');
                            text.extend_from_slice(itoa(field, &mut [0; 20]));
                        }
                        text.push(b'\n');
                        text.extend_from_within(..prefix);
                    }
                    text.truncate(text.len() - prefix);
                    if sender.send((hour, text)).is_err() {
                        return;
                    }
                }
            });
        }
        drop(sender);
        let (mut pending, mut written) = (vec![None; HOURS], 0);
        for (hour, text) in receiver {
            pending[hour] = Some(text);
            while let Some(text) = pending.get_mut(written).and_then(Option::take) {
                out.write_all(&text)?;
                written += 1;
            }
        }
        Ok::<(), std::io::Error>(())
    })?;
    // The caller exits right away, so the worker state is not freed.
    std::mem::forget((states, global));
    Ok(())
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
    let data = map_file(data.join("requests.log"))?;
    fs::create_dir_all(&out)?;
    summarize(data, workers, &mut File::create(out.join("summary.txt"))?)?;
    // Skip unmapping the input.
    std::process::exit(0);
}
