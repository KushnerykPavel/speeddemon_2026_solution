use std::collections::HashSet;
use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize};
use std::thread;

const BOM: &[u8] = b"\xEF\xBB\xBF";
/// Name table slots; the table counts as full at half of them (the generator has ~100,000 names),
/// so dense name ids stay below 2^(SLOT_BITS - 1).
const SLOT_BITS: u32 = 18;
const IDS: usize = 1 << (SLOT_BITS - 1);

unsafe extern "C" {
    fn mmap(
        address: *mut u8,
        length: usize,
        protection: i32,
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

/// Asks Linux for 4 KB pages (sparse buffers would commit whole 2 MB huge pages) and a fixed
/// glibc mmap threshold so large freed blocks go back to the kernel. Failures only cost memory.
fn tune_memory() {
    #[cfg(target_os = "linux")]
    unsafe {
        prctl(41, 1, 0, 0, 0);
        mallopt(-3, 256 << 10);
    }
}

/// Maps `path` read-only for the rest of the run (the data mount is read-only).
fn map_file(path: &Path) -> std::io::Result<&'static [u8]> {
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

/// The bytes of `name` as little-endian words, the last one padded with zeros.
fn words(name: &[u8]) -> impl Iterator<Item = u64> + '_ {
    name.chunks(8).map(|chunk| {
        let mut word = [0; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        u64::from_le_bytes(word)
    })
}

/// Distinct names, shared by all threads without locks. Names are kept in their raw (still
/// escaped) form, which is one-to-one with the value. A thread claims a free slot by swapping its
/// tag from 0 to 1, copies the name's words into the arena, then publishes the tag (a hash with
/// the top bit set). A name's id is its insertion number.
struct Table {
    tags: Vec<AtomicU64>,
    spans: Vec<AtomicU64>,
    arena: &'static [AtomicU64],
    next: AtomicUsize,
    used: AtomicUsize,
}

impl Table {
    fn new(bytes: usize) -> Self {
        let slots = 1 << SLOT_BITS;
        // Zeroed on demand by the kernel: only the pages names are copied to are charged.
        let arena = Box::leak(vec![0_u64; bytes / 8 + 1].into_boxed_slice());
        Self {
            tags: (0..slots).map(|_| AtomicU64::new(0)).collect(),
            spans: (0..slots).map(|_| AtomicU64::new(0)).collect(),
            // SAFETY: same size and alignment, and the words are only used through atomics now.
            arena: unsafe { std::slice::from_raw_parts(arena.as_ptr().cast(), arena.len()) },
            next: AtomicUsize::new(0),
            used: AtomicUsize::new(0),
        }
    }

    /// Returns the name's id, or `None` once the table is half full.
    fn id(&self, name: &[u8]) -> Option<usize> {
        let hash = words(name).fold(name.len() as u64, |state, word| {
            (state.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95)
        });
        let tag = hash | 1 << 63;
        let mask = (1 << SLOT_BITS) - 1;
        let mut slot = (hash >> 20) as usize & mask;
        loop {
            let mut seen = self.tags[slot].load(Acquire);
            if seen == 0 {
                if self.used.load(Relaxed) >= IDS || name.len() >= 1 << 16 {
                    return None;
                }
                if self.tags[slot]
                    .compare_exchange(0, 1, Acquire, Acquire)
                    .is_ok()
                {
                    let at = self.next.fetch_add(name.len().div_ceil(8), Relaxed);
                    for (cell, word) in self.arena[at..].iter().zip(words(name)) {
                        cell.store(word, Relaxed);
                    }
                    let id = self.used.fetch_add(1, Relaxed);
                    let span = (at as u64) << 33 | (id as u64) << 16 | name.len() as u64;
                    self.spans[slot].store(span, Relaxed);
                    self.tags[slot].store(tag, Release);
                    return Some(id);
                }
                seen = self.tags[slot].load(Acquire);
            }
            while seen == 1 {
                std::hint::spin_loop();
                seen = self.tags[slot].load(Acquire);
            }
            if seen == tag {
                let span = self.spans[slot].load(Relaxed);
                let at = (span >> 33) as usize;
                if span & 0xFFFF == name.len() as u64
                    && self.arena[at..]
                        .iter()
                        .zip(words(name))
                        .all(|(cell, word)| cell.load(Relaxed) == word)
                {
                    return Some((span >> 16 & 0x1_FFFF) as usize);
                }
            }
            slot = (slot + 1) & mask;
        }
    }
}

/// Running aggregates of one group.
#[derive(Clone, Copy)]
struct Group {
    count: u64,
    id_sum: u64,
    amount_sum: i64,
    min: i64,
    max: i64,
    name_bytes: u64,
    note_bytes: u64,
}

impl Default for Group {
    fn default() -> Self {
        Self {
            count: 0,
            id_sum: 0,
            amount_sum: 0,
            min: i64::MAX,
            max: i64::MIN,
            name_bytes: 0,
            note_bytes: 0,
        }
    }
}

/// Reads one field at `at`; returns its raw bytes (inside the quotes, `""` still doubled), its
/// value's length and the index after the separator that follows it.
fn field(bytes: &[u8], at: usize) -> (&[u8], usize, usize) {
    if bytes.get(at) != Some(&b'"') {
        let end = at + bytes[at..].iter().position(|&byte| byte == b',').unwrap();
        return (&bytes[at..end], end - at, end + 1);
    }
    let (mut from, mut escapes) = (at + 1, 0);
    loop {
        let quote = from + bytes[from..].iter().position(|&byte| byte == b'"').unwrap();
        if bytes.get(quote + 1) != Some(&b'"') {
            return (&bytes[at + 1..quote], quote - at - 1 - escapes, quote + 2);
        }
        escapes += 1;
        from = quote + 2;
    }
}

/// Decimal digits from `at` up to `stop`; returns the value and the index after `stop`.
fn digits(bytes: &[u8], mut at: usize, stop: u8) -> (u64, usize) {
    let mut value = 0_u64;
    while bytes[at] != stop {
        value = value
            .wrapping_mul(10)
            .wrapping_add(u64::from(bytes[at] - b'0'));
        at += 1;
    }
    (value, at + 1)
}

/// One parsed record: id, raw name, name and note value lengths, region and amount in cents.
type Record<'a> = (u64, &'a [u8], usize, usize, u64, i64);

/// Calls `visit` for every record in `bytes`, which starts at a record and ends after a
/// terminator or at the end of the file.
fn records<'a>(bytes: &'a [u8], mut visit: impl FnMut(Record<'a>)) {
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at..].starts_with(BOM) {
            at += 3;
        }
        let (id, next) = digits(bytes, at, b',');
        let (name, name_length, next) = field(bytes, next);
        let (_, note_length, next) = field(bytes, next);
        let (region, next) = digits(bytes, next, b',');
        let negative = bytes[next] == b'-';
        let (whole, next) = digits(bytes, next + usize::from(negative), b'.');
        let cents = (whole * 100
            + u64::from(bytes[next] - b'0') * 10
            + u64::from(bytes[next + 1] - b'0')) as i64;
        at = next + 2;
        at += usize::from(bytes.get(at) == Some(&b'\r'));
        at += usize::from(bytes.get(at) == Some(&b'\n'));
        visit((
            id,
            name,
            name_length,
            note_length,
            region,
            if negative { -cents } else { cents },
        ));
    }
}

/// Splits `bytes` (records only) into up to `pieces` chunks that each start at a record: after an
/// LF followed by an optional BOM, digits and a comma. Under the generator's guarantees (spec 2.2)
/// no line inside a quoted note looks like that.
fn chunks(bytes: &[u8], pieces: usize) -> Vec<&[u8]> {
    let record = |rest: &[u8]| {
        let rest = rest.strip_prefix(BOM).unwrap_or(rest);
        let digits = rest.iter().take_while(|byte| byte.is_ascii_digit()).count();
        digits > 0 && rest.get(digits) == Some(&b',')
    };
    let pieces = pieces.min(bytes.len() / (64 << 10)).max(1);
    let mut starts = vec![0];
    for piece in 1..pieces {
        let mut at = (bytes.len() * piece / pieces).max(*starts.last().unwrap());
        loop {
            match bytes[at..].iter().position(|&byte| byte == b'\n') {
                Some(offset) => at += offset + 1,
                None => at = bytes.len(),
            }
            if at == bytes.len() || record(&bytes[at..]) {
                break;
            }
        }
        starts.push(at);
    }
    starts.push(bytes.len());
    starts.dedup();
    starts
        .windows(2)
        .map(|pair| &bytes[pair[0]..pair[1]])
        .collect()
}

fn money(text: &mut Vec<u8>, cents: i64) {
    if cents < 0 {
        text.push(b'-');
    }
    let cents = cents.unsigned_abs();
    write!(text, "{}.{:02}", cents / 100, cents % 100).unwrap();
}

fn main() -> Result<(), Box<dyn Error>> {
    tune_memory();
    let (mut data, mut out) = (PathBuf::from("/data"), PathBuf::from("/out"));
    let mut workers = thread::available_parallelism()
        .map_or(1, usize::from)
        .min(12);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("missing {arg} value"));
        match arg.as_str() {
            "--data" => data = value()?.into(),
            "--out" => out = value()?.into(),
            "--threads" => workers = value()?.parse::<std::num::NonZeroUsize>()?.get().min(12),
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    // Regions: `region_id,country` lines after the header; codes are three capital letters, so
    // a code's number in base 26 orders codes like their bytes.
    let mut rows = Vec::new();
    let mut present = vec![false; 26 * 26 * 26];
    for line in map_file(&data.join("regions.csv"))?
        .split(|&byte| byte == b'\n')
        .skip(1)
    {
        if let [.., b',', a, b, c] = *line {
            let code = [a, b, c]
                .iter()
                .fold(0, |code, &letter| code * 26 + usize::from(letter - b'A'));
            present[code] = true;
            rows.push((digits(line, 0, b',').0 as usize, code));
        }
    }
    let codes: Vec<usize> = (0..present.len()).filter(|&code| present[code]).collect();
    let groups = codes.len() + 1;
    let mut country = vec![
        codes.len() as u16;
        rows.iter()
            .map(|&(region, _)| region + 1)
            .max()
            .unwrap_or(0)
    ];
    for &(region, code) in &rows {
        country[region] = codes.binary_search(&code).unwrap() as u16;
    }
    // Records: skip a leading BOM and the header line.
    let records_file = map_file(&data.join("records.csv"))?;
    let file = records_file.strip_prefix(BOM).unwrap_or(records_file);
    let body = &file[file
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(file.len(), |at| at + 1)..];
    let table = Table::new(body.len());
    let full = AtomicBool::new(false);
    let group_of = |region: u64| {
        country
            .get(region as usize)
            .map_or(groups - 1, |&group| group as usize)
    };
    let words = IDS / 64;
    let names: Vec<AtomicU64> = (0..groups * words).map(|_| AtomicU64::new(0)).collect();
    let parts: Vec<Vec<Group>> = thread::scope(|scope| {
        let handles: Vec<_> = chunks(body, workers)
            .into_iter()
            .map(|chunk| {
                let (table, names, full, group_of) = (&table, &names, &full, &group_of);
                scope.spawn(move || {
                    let mut part = vec![Group::default(); groups];
                    records(
                        chunk,
                        |(id, name, name_length, note_length, region, amount)| {
                            let index = group_of(region);
                            let group = &mut part[index];
                            group.count += 1;
                            group.id_sum += id;
                            group.amount_sum += amount;
                            group.min = group.min.min(amount);
                            group.max = group.max.max(amount);
                            group.name_bytes += name_length as u64;
                            group.note_bytes += note_length as u64;
                            match table.id(name) {
                                Some(id) => {
                                    _ = names[index * words + id / 64]
                                        .fetch_or(1 << (id % 64), Relaxed)
                                }
                                None => full.store(true, Relaxed),
                            }
                        },
                    );
                    part
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    // More names than the table holds: count distinct names exactly in one slow pass instead.
    let mut exact = vec![HashSet::new(); if full.into_inner() { groups } else { 0 }];
    if !exact.is_empty() {
        records(body, |(_, name, _, _, region, _)| {
            _ = exact[group_of(region)].insert(name)
        });
    }
    let mut text = Vec::new();
    for index in 0..groups {
        let mut total = Group::default();
        for group in parts.iter().map(|part| &part[index]) {
            total.count += group.count;
            total.id_sum += group.id_sum;
            total.amount_sum += group.amount_sum;
            total.min = total.min.min(group.min);
            total.max = total.max.max(group.max);
            total.name_bytes += group.name_bytes;
            total.note_bytes += group.note_bytes;
        }
        if total.count == 0 {
            continue;
        }
        let distinct = match exact.get(index) {
            Some(set) => set.len() as u64,
            None => names[index * words..(index + 1) * words]
                .iter()
                .map(|word| u64::from(word.load(Relaxed).count_ones()))
                .sum(),
        };
        match codes.get(index) {
            Some(&code) => text
                .extend([code / 676, code / 26 % 26, code % 26].map(|letter| b'A' + letter as u8)),
            None => text.extend_from_slice(b"_UNMATCHED"),
        }
        write!(text, " {} {} ", total.count, total.id_sum)?;
        money(&mut text, total.amount_sum);
        text.push(b' ');
        money(&mut text, total.min);
        text.push(b' ');
        money(&mut text, total.max);
        writeln!(
            text,
            " {distinct} {} {}",
            total.name_bytes, total.note_bytes
        )?;
    }
    fs::create_dir_all(&out)?;
    fs::write(out.join("aggregates.txt"), text)?;
    Ok(())
}
