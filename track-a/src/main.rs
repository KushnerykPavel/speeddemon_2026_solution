use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

// The spec fixes every timestamp between 2026-10-30T18 and 2026-11-02T17 (72 hours).
const HOURS: usize = 72;
const DATES: [&str; 4] = ["2026-10-30", "2026-10-31", "2026-11-01", "2026-11-02"];
const LANES: u64 = 0x0101_0101_0101_0101;
const BAD: &str = "malformed log record";
// Readable bytes the parser may touch past the end of a chunk.
const SLACK: usize = 64;

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

// A group's latencies in a chunk: `count` values of endpoint `endpoint`.
#[derive(Clone, Copy)]
struct Run {
    endpoint: u32,
    count: u32,
    errors: u32,
}

// The records of one chunk, grouped by (hour, endpoint). Hour `h` owns
// `runs[run_at[h]..run_at[h + 1]]` and `lats[lat_at[h]..lat_at[h + 1]]`.
struct Chunk {
    lats: Vec<u32>,
    runs: Vec<Run>,
    lat_at: [u32; HOURS + 1],
    run_at: [u32; HOURS + 1],
}

// One worker: an endpoint table (open addressing on a 64-bit hash of all the
// endpoint bytes; a slot holds the hash's high 48 bits and the ID plus one)
// and the chunks it has parsed.
struct State {
    names: Vec<&'static [u8]>,
    hashes: Vec<u64>,
    slots: Vec<u64>,
    shift: u32,
    chunks: Vec<Chunk>,
}

impl State {
    fn new() -> Self {
        State {
            names: Vec::new(),
            hashes: Vec::new(),
            slots: vec![0; 8192],
            shift: 51,
            chunks: Vec::new(),
        }
    }

    fn endpoint(&mut self, name: &'static [u8], hash: u64) -> Result<u64, &'static str> {
        let mask = self.slots.len() - 1;
        let mut i = (hash >> self.shift) as usize;
        loop {
            let slot = self.slots[i];
            if slot == 0 {
                break;
            }
            if (slot ^ hash) >> 16 == 0 {
                return Ok((slot & 0xffff) - 1);
            }
            i = (i + 1) & mask;
        }
        if self.names.len() >= 0xfffe {
            return Err("too many endpoints");
        }
        // Own the name: the input pages it came from are unmapped after parsing.
        self.names.push(Box::leak(name.to_vec().into_boxed_slice()));
        self.hashes.push(hash);
        self.slots[i] = hash & !0xffff | self.names.len() as u64;
        if self.names.len() * 2 > self.slots.len() {
            self.slots = vec![0; self.slots.len() * 2];
            self.shift -= 1;
            let mask = self.slots.len() - 1;
            for (id, hash) in self.hashes.iter().enumerate() {
                let mut i = (hash >> self.shift) as usize;
                while self.slots[i] != 0 {
                    i = (i + 1) & mask;
                }
                self.slots[i] = hash & !0xffff | id as u64 + 1;
            }
        }
        Ok(self.names.len() as u64 - 1)
    }

    // Parses the lines `data[start..end]` (`SLACK` readable bytes follow) into one chunk.
    fn chunk(&mut self, data: &'static [u8], start: usize, end: usize) -> Result<(), &'static str> {
        // A record takes at least 38 bytes, so `keys` never reallocates; untouched
        // capacity is not resident.
        let mut keys = Vec::with_capacity((end - start) / 38 + 1);
        let (first, last) = self.scan(data, start, end, &mut keys)?;
        let (endpoints, span) = (self.names.len(), last + 1 - first);
        let group = |key: u64| {
            ((key >> 56 & 0x7f) as usize - first) * endpoints + (key >> 32 & 0xff_ffff) as usize
        };
        let mut at = vec![0_u32; span * endpoints + 1];
        let mut errors = vec![0_u32; span * endpoints];
        for &key in &keys {
            at[group(key) + 1] += 1;
            errors[group(key)] += (key >> 63) as u32;
        }
        for g in 0..span * endpoints {
            at[g + 1] += at[g];
        }
        let mut cursor = at.clone();
        let mut lats = vec![0; keys.len()];
        for &key in &keys {
            let g = group(key);
            lats[cursor[g] as usize] = key as u32;
            cursor[g] += 1;
        }
        let mut chunk = Chunk {
            lats,
            runs: Vec::new(),
            lat_at: [0; HOURS + 1],
            run_at: [0; HOURS + 1],
        };
        for hour in 0..=HOURS {
            let k = hour.clamp(first, last + 1) - first;
            chunk.lat_at[hour] = at[k * endpoints];
            chunk.run_at[hour] = chunk.runs.len() as u32;
            if hour >= first && k < span {
                for e in 0..endpoints {
                    let g = k * endpoints + e;
                    if at[g + 1] > at[g] {
                        let (endpoint, count, errors) = (e as u32, at[g + 1] - at[g], errors[g]);
                        chunk.runs.push(Run {
                            endpoint,
                            count,
                            errors,
                        });
                    }
                }
            }
        }
        chunk.runs.shrink_to_fit();
        self.chunks.push(chunk);
        Ok(())
    }

    // Parses the whole lines `data[pos..end]`, `timestamp method endpoint status latency`,
    // into keys `error << 63 | hour << 56 | endpoint << 32 | latency`; returns the hour range.
    // The line end comes first (64 bytes per step); the latency, status and endpoint end are
    // then read backwards from it, so the endpoint's bytes are only hashed, never searched.
    fn scan(
        &mut self,
        data: &'static [u8],
        pos: usize,
        end: usize,
        keys: &mut Vec<u64>,
    ) -> Result<(usize, usize), &'static str> {
        assert!(pos <= end && end + SLACK <= data.len());
        let (mut p, stop) = unsafe { (data.as_ptr().add(pos), data.as_ptr().add(end)) };
        let (mut first, mut last, mut stamp, mut hour) = (HOURS, 0, 0, HOURS);
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
                    (stamp, first, last) = (w, first.min(hour), last.max(hour));
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
                let id = self.endpoint(
                    std::slice::from_raw_parts(name, length),
                    hash ^ length as u64,
                )?;
                let error = u64::from(*tail.add(1) == b'5');
                keys.push(error << 63 | (hour as u64) << 56 | id << 32 | latency);
            }
        }
        Ok((first.min(last), last))
    }
}

// The competition target and local development hosts use 64-bit Unix off_t.
unsafe extern "C" {
    fn munmap(address: *mut u8, length: usize) -> i32;
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

// Unmaps the whole pages inside the parsed chunk `data[start..end]`, keeping one page at
// each end because a neighbouring chunk's parser may read past its end. Doing it here, in
// parallel, spares the kernel one serial teardown of the whole mapping at exit.
fn release(data: &[u8], start: usize, end: usize) {
    const PAGE: usize = 16384; // a multiple of every supported page size
    let (from, to) = (
        (start.div_ceil(PAGE) + 1) * PAGE,
        (end / PAGE).saturating_sub(1) * PAGE,
    );
    if to > from {
        unsafe { munmap(data.as_ptr().add(from) as *mut u8, to - from) };
    }
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

// The rows of one hour: endpoint rank, count, p50, p95, p99, errors.
type Rows = Vec<[u32; 6]>;

fn summarize(
    data: &'static [u8],
    workers: usize,
    out: &mut (impl Write + Send),
) -> Result<(), Box<dyn Error>> {
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
    let next = AtomicUsize::new(0);
    let states = run(workers, || {
        let mut state = State::new();
        while let Some(&(start, end)) = chunks.get(next.fetch_add(1, Ordering::Relaxed)) {
            match start {
                usize::MAX => state.chunk(tail, 0, end)?,
                _ => {
                    state.chunk(data, start, end)?;
                    release(data, start, end);
                }
            }
        }
        Ok::<State, &'static str>(state)
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;

    // Global endpoint table, built from the workers' stored hashes; ranks follow byte order.
    let mut global = State::new();
    let mut local_ids = Vec::new();
    for state in &states {
        let ids: Result<Vec<u64>, _> = (state.names.iter().zip(&state.hashes))
            .map(|(&name, &hash)| global.endpoint(name, hash))
            .collect();
        local_ids.push(ids?);
    }
    let mut order: Vec<usize> = (0..global.names.len()).collect();
    order.sort_unstable_by_key(|&id| global.names[id]);
    let endpoints: Vec<&[u8]> = order.iter().map(|&id| global.names[id]).collect();
    let mut rank_of = vec![0; order.len()];
    for (rank, &id) in order.iter().enumerate() {
        rank_of[id] = rank as u32;
    }
    // remap[state][local endpoint ID] is the endpoint's rank in byte order.
    let remap: Vec<Vec<u32>> = (local_ids.iter())
        .map(|ids| ids.iter().map(|&id| rank_of[id as usize]).collect())
        .collect();

    // Quantiles of every group, hour by hour.
    let next = AtomicUsize::new(0);
    let ranks = endpoints.len();
    let mut rows: Vec<(usize, Rows)> = run(workers, || {
        let (mut done, mut count, mut errors) =
            (Vec::new(), vec![0_u32; ranks + 1], vec![0_u32; ranks]);
        let (mut cursor, mut lats) = (vec![0_u32; ranks], Vec::new());
        loop {
            let hour = next.fetch_add(1, Ordering::Relaxed);
            if hour >= HOURS {
                return done;
            }
            // Every run of this hour with its endpoint rank, error count and latencies.
            let runs = || {
                states.iter().zip(&remap).flat_map(move |(state, ranks)| {
                    state.chunks.iter().flat_map(move |c| {
                        let range = c.run_at[hour] as usize..c.run_at[hour + 1] as usize;
                        let mut at = c.lat_at[hour] as usize;
                        c.runs[range].iter().map(move |run| {
                            let values = &c.lats[at..at + run.count as usize];
                            at += run.count as usize;
                            (ranks[run.endpoint as usize] as usize, run.errors, values)
                        })
                    })
                })
            };
            count.fill(0);
            errors.fill(0);
            for (rank, errs, values) in runs() {
                count[rank + 1] += values.len() as u32;
                errors[rank] += errs;
            }
            for rank in 0..ranks {
                count[rank + 1] += count[rank];
            }
            cursor.copy_from_slice(&count[..ranks]);
            lats.resize(count[ranks] as usize, 0);
            for (rank, _, values) in runs() {
                lats[cursor[rank] as usize..][..values.len()].copy_from_slice(values);
                cursor[rank] += values.len() as u32;
            }
            let mut hour_rows = Vec::new();
            for rank in 0..ranks {
                let group = &mut lats[count[rank] as usize..count[rank + 1] as usize];
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
                hour_rows.push([
                    rank as u32,
                    n as u32,
                    values[0],
                    values[1],
                    values[2],
                    errors[rank],
                ]);
            }
            done.push((hour, hour_rows));
        }
    })
    .concat();
    // The parsed records are no longer needed; free them so they and the output
    // file's pages are not in memory together.
    drop((states, global));
    rows.sort_unstable_by_key(|&(hour, _)| hour);
    let rows: Vec<Rows> = rows.into_iter().map(|(_, rows)| rows).collect();

    // Workers format whole hours and write them in order as they complete.
    let next = AtomicUsize::new(0);
    let sink = Mutex::new((0, vec![None; HOURS], out));
    run(workers, || {
        loop {
            let hour = next.fetch_add(1, Ordering::Relaxed);
            if hour >= HOURS {
                return;
            }
            let date = DATES[(hour + 18) / 24];
            let mut text = format!("{date}T{:02}:00:00Z ", (hour + 18) % 24).into_bytes();
            let prefix = text.len();
            for &[rank, n, p50, p95, p99, errors] in &rows[hour] {
                text.extend_from_slice(endpoints[rank as usize]);
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
                text.extend_from_within(..prefix);
            }
            text.truncate(text.len() - prefix);
            // Write this hour, and any later ones already finished, if it is next.
            let mut guard = sink.lock().unwrap();
            let (written, pending, out) = &mut *guard;
            pending[hour] = Some(text);
            while let Some(text) = pending.get_mut(*written).and_then(Option::take) {
                out.write_all(&text).expect("cannot write summary.txt");
                *written += 1;
            }
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
    summarize(data, workers, &mut File::create(out.join("summary.txt"))?)?;
    // Skip unmapping the input.
    std::process::exit(0);
}
