use std::error::Error;
use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::thread;

// The spec fixes every timestamp between 2026-10-30T18 and 2026-11-02T17 (72 hours).
const HOURS: usize = 72;
const DATES: [&str; 4] = ["2026-10-30", "2026-10-31", "2026-11-01", "2026-11-02"];
const LANES: u64 = 0x0101_0101_0101_0101;
const BAD: &str = "malformed log record";
// Readable bytes the parser may touch past the end of a chunk.
const SLACK: usize = 64;
// Endpoint table slots (a seed has 1,842 endpoints) and the record layout: latency above
// a 13-bit key (12-bit slot, 5xx flag); latencies of 2^19 or more go on a side list.
const SLOTS: usize = 1 << 12;
const PACKED: u32 = 1 << 19;
// Records per block: with `hour` and `n`, a block is 16 KB.
const CAP: usize = 4094;

unsafe fn load(at: *const u8) -> u64 {
    unsafe { at.cast::<u64>().read_unaligned() }
}

// Bit `i` of the result is set when byte `i` of the 64 bytes at `at` is a newline.
unsafe fn newlines(at: *const u8) -> u64 {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::*;
        let newline = _mm_set1_epi8(b'\n' as i8);
        let part = |i: usize| {
            let bytes = _mm_loadu_si128(at.add(16 * i).cast());
            _mm_movemask_epi8(_mm_cmpeq_epi8(bytes, newline)) as u16 as u64
        };
        part(0) | part(1) << 16 | part(2) << 32 | part(3) << 48
    }
    #[cfg(not(target_arch = "x86_64"))]
    (0..64).fold(0, |m, i| m | u64::from(unsafe { *at.add(i) } == b'\n') << i)
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

// Decimal number of up to ten digits at `at` and its length; the caller
// guarantees `SLACK` readable bytes.
unsafe fn number(at: *const u8) -> Option<(u64, usize)> {
    let w = unsafe { load(at) };
    let length = ((!w & (0x10 * LANES)).trailing_zeros() / 8) as usize;
    if length < 8 {
        let t = (w ^ (0x30 * LANES)) & ((1_u64 << (8 * length)) - 1);
        if length == 0 || t & (0xf0 * LANES) != 0 || (t + 6 * LANES) & (0xf0 * LANES) != 0 {
            return None;
        }
        let t = (t << (8 * (8 - length))).wrapping_mul(2561) >> 8;
        let t = (t & 0x00ff_00ff_00ff_00ff).wrapping_mul(6_553_601) >> 16;
        let t = (t & 0x0000_ffff_0000_ffff).wrapping_mul(42_949_672_960_001) >> 32;
        return Some((t, length));
    }
    let (mut value, mut length) = (0_u64, 0);
    while let Some(d) = unsafe { *at.add(length) }
        .checked_sub(b'0')
        .filter(|d| *d < 10)
    {
        value = value * 10 + u64::from(d);
        length += 1;
        if length > 10 || value > u64::from(u32::MAX) {
            return None;
        }
    }
    Some((value, length))
}

// 16 KB of one hour's records from one thread.
struct Block {
    hour: u32,
    n: u32,
    rec: [u32; CAP],
}

// Endpoint names to slots, one table for all threads. A slot holds the 64-bit hash of the
// whole name (never zero) and, once the thread that claimed it has copied the name, the
// name itself. A name's id is its slot.
struct Names {
    hashes: Vec<AtomicU64>,
    names: Vec<OnceLock<&'static [u8]>>,
}

impl Names {
    fn slot(&self, name: &[u8], hash: u64) -> Result<u32, &'static str> {
        let mut i = (hash >> 52) as usize;
        for _ in 0..SLOTS {
            let found = self.hashes[i].load(Relaxed);
            if found == hash {
                return Ok(i as u32);
            }
            if found == 0 {
                match self.hashes[i].compare_exchange(0, hash, Relaxed, Relaxed) {
                    Ok(_) => {
                        // The input pages `name` lives in are dropped after parsing.
                        let _ = self.names[i].set(Box::leak(name.into()));
                        return Ok(i as u32);
                    }
                    Err(other) if other == hash => return Ok(i as u32),
                    Err(_) => {}
                }
            }
            i = (i + 1) % SLOTS;
        }
        Err("too many endpoints")
    }
}

// One parser thread's records, by hour: `index[h]` lists the blocks of hour `h`.
struct State {
    blocks: Vec<Block>,
    open: [u32; HOURS],
    index: Vec<Vec<u32>>,
    wide: Vec<Vec<(u32, u32)>>,
    count: [u32; HOURS],
    // The first block not yet released.
    kept: AtomicUsize,
}

impl State {
    fn new(room: usize) -> Self {
        State {
            blocks: Vec::with_capacity(room),
            open: [u32::MAX; HOURS],
            index: vec![Vec::new(); HOURS],
            wide: vec![Vec::new(); HOURS],
            count: [0; HOURS],
            kept: AtomicUsize::new(0),
        }
    }

    fn push(&mut self, hour: usize, latency: u32, key: u32) {
        self.count[hour] += 1;
        if latency >= PACKED {
            return self.wide[hour].push((latency, key));
        }
        let mut b = self.open[hour] as usize;
        if b >= self.blocks.len() || self.blocks[b].n as usize == CAP {
            // `room` covers every block this thread can need, so this never reallocates.
            assert!(self.blocks.len() < self.blocks.capacity());
            b = self.blocks.len();
            self.open[hour] = b as u32;
            self.index[hour].push(b as u32);
            self.blocks.push(Block {
                hour: hour as u32,
                n: 0,
                rec: [0; CAP],
            });
        }
        let block = &mut self.blocks[b];
        block.rec[block.n as usize] = latency << 13 | key;
        block.n += 1;
    }

    // Parses the whole lines `data[pos..end]`, `timestamp method endpoint status latency`.
    // The line end comes first (64 bytes per step); the latency, status and endpoint end are
    // then read backwards from it, so the endpoint's bytes are only hashed, never searched.
    fn scan(
        &mut self,
        names: &Names,
        data: &[u8],
        pos: usize,
        end: usize,
    ) -> Result<(), &'static str> {
        assert!(pos <= end && end + SLACK <= data.len());
        let (mut p, stop) = unsafe { (data.as_ptr().add(pos), data.as_ptr().add(end)) };
        let (mut stamp, mut hour) = (0, HOURS);
        let (mut block, mut mask) = (p, 0_u64);
        // SAFETY: a line starts inside the chunk and is at least 37 bytes long; every read
        // below stays within 64 bytes of a position inside the line or the chunk's end.
        unsafe {
            while p < stop {
                // The next newline, or the chunk's end for an unterminated last line.
                let newline = loop {
                    if mask != 0 {
                        let at = block.sub(64).add(mask.trailing_zeros() as usize);
                        mask &= mask - 1;
                        break at;
                    }
                    if block >= stop {
                        break stop;
                    }
                    mask = newlines(block);
                    block = block.add(64);
                };
                let line_end = if newline > p && *newline.sub(1) == b'\r' {
                    newline.sub(1)
                } else {
                    newline
                };
                if line_end.offset_from(p) < 37 {
                    return Err(BAD);
                }
                let w = load(p.add(5));
                // Records come in long runs of one hour: decode the hour only when it changes.
                if w != stamp || hour >= HOURS {
                    let two = |at: u32| ((w >> at & 15) * 10 + (w >> (at + 8) & 15)) as i64;
                    hour = ((two(24) + 1 - 31 * (11 - two(0))) * 24 + two(48) - 18) as usize;
                    if hour >= HOURS || !matches!(two(0), 10 | 11) {
                        return Err(BAD);
                    }
                    stamp = w;
                }
                let method = space_in(load(p.add(28))).filter(|&m| m > 0).ok_or(BAD)?;
                let name = p.add(29 + method);
                // The latency's digits end the line; find the space before them.
                let x = load(line_end.sub(8)) ^ (0x20 * LANES);
                let spaces = !(((x & (0x7f * LANES)) + (0x7f * LANES)) | x | (0x7f * LANES));
                let mut digits = (spaces.leading_zeros() / 8) as usize;
                if spaces == 0 {
                    digits = 8;
                    while digits <= 10 && *line_end.sub(digits + 1) != b' ' {
                        digits += 1;
                    }
                }
                let (latency, found) = match digits {
                    1..=10 => number(line_end.sub(digits)).ok_or(BAD)?,
                    _ => return Err(BAD),
                };
                // " SSS " precedes the digits: a space, three status digits, a space.
                let tail = line_end.sub(digits + 5);
                let x = (load(tail) ^ 0x20_3030_3020) & 0xff_ffff_ffff;
                let bad = x & 0xff_0000_00ff
                    | x & 0xf0f0_f000
                    | ((x & 0x0f0f_0f00) + 0x0606_0600) & 0x1010_1000;
                if found != digits || bad != 0 || tail <= name {
                    return Err(BAD);
                }
                let (mut at, mut hash) = (name, 0);
                while at.add(8) <= tail {
                    hash = hash_word(hash, load(at));
                    at = at.add(8);
                }
                let rest = tail.offset_from(at) as usize;
                hash = hash_word(hash, load(at) & ((1_u64 << (8 * rest)) - 1));
                let length = tail.offset_from(name) as usize;
                p = newline.add(1);
                let name = std::slice::from_raw_parts(name, length);
                let slot = names.slot(name, (hash ^ length as u64) | 1)?;
                let error = u32::from(*tail.add(1) == b'5');
                self.push(hour, latency as u32, error << 12 | slot);
            }
        }
        Ok(())
    }
}

// The competition target and local development hosts use 64-bit Unix off_t.
unsafe extern "C" {
    fn madvise(address: *mut u8, length: usize, advice: i32) -> i32;
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

// 4 KB pages instead of transparent huge pages, and blocks of 64 KB or more are
// mapped on their own so freeing them returns the memory. Failures only cost memory.
fn tune_memory() {
    #[cfg(target_os = "linux")]
    unsafe {
        prctl(41, 1, 0, 0, 0);
        mallopt(-3, 64 << 10);
    }
}

// Gives the kernel back the whole pages inside `from..to`, whose contents are never read
// again. MADV_DONTNEED takes the address-space lock shared, so other threads' page
// faults go on.
fn drop_pages(from: *const u8, to: *const u8) {
    const PAGE: usize = 4096;
    let (from, to) = (
        (from as usize).next_multiple_of(PAGE),
        to as usize / PAGE * PAGE,
    );
    if to > from {
        unsafe { madvise(from as *mut u8, to - from, 4) };
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

// Appends `value` in decimal, two digits per step.
fn push_number(text: &mut Vec<u8>, mut value: u64) {
    const PAIRS: &[u8; 200] = b"0001020304050607080910111213141516171819202122232425262728293031323334353637383940414243444546474849\
5051525354555657585960616263646566676869707172737475767778798081828384858687888990919293949596979899";
    let mut digits = [0; 20];
    let mut at = digits.len();
    while value >= 100 {
        let pair = (value % 100) as usize * 2;
        value /= 100;
        at -= 2;
        digits[at..at + 2].copy_from_slice(&PAIRS[pair..pair + 2]);
    }
    if value >= 10 {
        at -= 2;
        digits[at..at + 2].copy_from_slice(&PAIRS[value as usize * 2..][..2]);
    } else {
        at -= 1;
        digits[at] = b'0' + value as u8;
    }
    text.extend_from_slice(&digits[at..]);
}

// One group's output numbers: endpoint rank, count, p50, p95, p99, errors.
type Row = [u32; 6];

// Hands back the blocks at the front of each thread's list whose hours are all finished
// (the log is in arrival order, so they empty roughly in order), in batches of 64 KB.
fn release(states: &[State], finished: &[AtomicBool]) {
    for state in states {
        let (from, mut to) = (state.kept.load(Relaxed), state.kept.load(Relaxed));
        while to < state.blocks.len() && finished[state.blocks[to].hour as usize].load(Relaxed) {
            to += 1;
        }
        if to - from >= 4 || (to == state.blocks.len() && to > from) {
            let base = state.blocks.as_ptr();
            drop_pages(base.wrapping_add(from).cast(), base.wrapping_add(to).cast());
            state.kept.store(to, Relaxed);
        }
    }
}

fn summarize(data: &'static [u8], workers: usize, out: &File) -> Result<(), Box<dyn Error>> {
    // The parser reads up to `SLACK` bytes past a chunk, so the last few lines are copied
    // into a zero-padded buffer and parsed from there.
    let cut = match data.len().checked_sub(512).filter(|_| data.len() >= 1024) {
        Some(from) => data[from..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(data.len(), |i| from + i + 1),
        None => 0,
    };
    let mut tail = data[cut..].to_vec();
    let tail_len = tail.len();
    tail.resize(tail_len + SLACK, 0);
    let tail: &'static [u8] = Box::leak(tail.into_boxed_slice());
    let mut chunks = line_chunks(&data[..cut], workers * 8);
    chunks.push((usize::MAX, tail_len));

    // Parse: each thread files 4-byte records by hour; endpoints go to the shared table.
    let names = Names {
        hashes: (0..SLOTS).map(|_| AtomicU64::new(0)).collect(),
        names: (0..SLOTS).map(|_| OnceLock::new()).collect(),
    };
    let room = data.len() / 37 / CAP + HOURS + 2;
    let next = AtomicUsize::new(0);
    let states = run(workers, || {
        let mut state = State::new(room);
        while let Some(&(start, end)) = chunks.get(next.fetch_add(1, Relaxed)) {
            match start {
                usize::MAX => state.scan(&names, tail, 0, end)?,
                _ => {
                    state.scan(&names, data, start, end)?;
                    // Keep a page at each end: the neighbouring piece's parser reads past its end.
                    let (from, to) = (
                        data.as_ptr().wrapping_add(start),
                        data.as_ptr().wrapping_add(end),
                    );
                    drop_pages(from.wrapping_add(8192), to.wrapping_sub(8192));
                }
            }
        }
        Ok::<State, &'static str>(state)
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;

    // Endpoint ranks in byte order, which is the order of the output lines.
    let mut order: Vec<usize> = (0..SLOTS)
        .filter(|&i| names.names[i].get().is_some())
        .collect();
    order.sort_unstable_by(|&a, &b| names.names[a].get().cmp(&names.names[b].get()));
    let mut rank = vec![0_u32; SLOTS];
    for (r, &slot) in order.iter().enumerate() {
        rank[slot] = r as u32;
    }
    let ne = order.len();
    let most = (0..HOURS)
        .map(|h| states.iter().map(|s| s.count[h] as usize).sum())
        .max();

    // Hours: a counting sort of the hour's latencies by endpoint rank, then three
    // selections per group. Each group's numbers are kept with the exact size of the text.
    let (finished, releasing) = (
        (0..HOURS)
            .map(|_| AtomicBool::new(false))
            .collect::<Vec<_>>(),
        AtomicBool::new(false),
    );
    let next = AtomicUsize::new(0);
    let mut hours: Vec<(usize, Vec<Row>, usize)> = run(workers, || {
        let (mut done, mut count, mut errors) = (Vec::new(), vec![0_u32; ne + 1], vec![0_u32; ne]);
        // Sized once and not zeroed: only the pages a thread's hours use are charged.
        let (mut cursor, mut lats) = (vec![0_u32; ne], vec![0_u32; most.unwrap_or(0)]);
        loop {
            let hour = next.fetch_add(1, Relaxed);
            if hour >= HOURS {
                return done;
            }
            // Every record of the hour as (endpoint rank, 5xx, latency).
            let records = || {
                states.iter().flat_map(move |s| {
                    let packed = s.index[hour].iter().flat_map(move |&b| {
                        let block = &s.blocks[b as usize];
                        block.rec[..block.n as usize]
                            .iter()
                            .map(|&r| (r & 0xfff, r >> 12 & 1, r >> 13))
                    });
                    packed.chain(
                        s.wide[hour]
                            .iter()
                            .map(|&(l, k)| (k & 0xfff, k >> 12 & 1, l)),
                    )
                })
            };
            count.fill(0);
            errors.fill(0);
            for (slot, error, _) in records() {
                let e = rank[slot as usize] as usize;
                (count[e + 1], errors[e]) = (count[e + 1] + 1, errors[e] + error);
            }
            for e in 0..ne {
                count[e + 1] += count[e];
            }
            cursor.copy_from_slice(&count[..ne]);
            for (slot, _, latency) in records() {
                let e = rank[slot as usize] as usize;
                lats[cursor[e] as usize] = latency;
                cursor[e] += 1;
            }
            finished[hour].store(true, Relaxed);
            if !releasing.swap(true, Relaxed) {
                release(&states, &finished);
                releasing.store(false, Relaxed);
            }
            let (mut rows, mut size) = (Vec::new(), 0);
            for e in 0..ne {
                let group = &mut lats[count[e] as usize..count[e + 1] as usize];
                if group.is_empty() {
                    continue;
                }
                let n = group.len();
                let mut at = [50, 95, 99].map(|p| (p * n).div_ceil(100) - 1);
                let (mut values, mut high) = ([0; 3], n);
                for (value, index) in values.iter_mut().zip(at.iter_mut()).rev() {
                    if *index < high {
                        group[..high].select_nth_unstable(*index);
                        high = *index;
                    }
                    *value = group[*index];
                }
                let row = [
                    e as u32, n as u32, values[0], values[1], values[2], errors[e],
                ];
                let ppm = (u64::from(errors[e]) * 1_000_000 / n as u64) as u32;
                // Label, name, six numbers each after a space, newline.
                size += 21 + names.names[order[e]].get().unwrap().len() + 7;
                size += [row[1], row[2], row[3], row[4], row[5], ppm]
                    .iter()
                    .map(|x| x.checked_ilog10().map_or(1, |d| d as usize + 1))
                    .sum::<usize>();
                rows.push(row);
            }
            done.push((hour, rows, size));
        }
    })
    .concat();
    hours.sort_unstable_by_key(|&(hour, ..)| hour);
    // The records are no longer needed; free them before the output's pages are charged.
    release(&states, &finished);
    drop(states);

    // Text: the output file is sized from the exact line sizes and mapped, and every thread
    // formats its hours straight into it.
    let total: usize = hours.iter().map(|h| h.2).sum();
    out.set_len(total as u64)?;
    if total == 0 {
        return Ok(());
    }
    let base = unsafe { mmap(std::ptr::null_mut(), total, 3, 1, out.as_raw_fd(), 0) };
    if base as isize == -1 {
        return Err(std::io::Error::last_os_error().into());
    }
    let at: Vec<usize> = hours
        .iter()
        .scan(0, |at, h| Some(std::mem::replace(at, *at + h.2)))
        .collect();
    let (base, next) = (base as usize, AtomicUsize::new(0));
    run(workers, || {
        let mut text = Vec::new();
        loop {
            let hour = next.fetch_add(1, Relaxed);
            if hour >= HOURS {
                return;
            }
            let label = format!(
                "{}T{:02}:00:00Z ",
                DATES[(hour + 18) / 24],
                (hour + 18) % 24
            );
            text.clear();
            for &[e, n, p50, p95, p99, errors] in &hours[hour].1 {
                text.extend_from_slice(label.as_bytes());
                text.extend_from_slice(names.names[order[e as usize]].get().unwrap());
                let ppm = u64::from(errors) * 1_000_000 / u64::from(n);
                for field in [
                    n.into(),
                    p50.into(),
                    p95.into(),
                    p99.into(),
                    errors.into(),
                    ppm,
                ] {
                    text.push(b' ');
                    push_number(&mut text, field);
                }
                text.push(b'\n');
            }
            assert_eq!(text.len(), hours[hour].2);
            // SAFETY: the hours' ranges are disjoint and lie inside the mapping.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    text.as_ptr(),
                    (base + at[hour]) as *mut u8,
                    text.len(),
                )
            };
        }
    });
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
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(out.join("summary.txt"))?;
    summarize(data, workers, &file)?;
    // Skip unmapping the input and the output.
    std::process::exit(0);
}
