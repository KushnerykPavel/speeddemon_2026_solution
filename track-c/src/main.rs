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
    /// Per slot: the tag, then the name's arena offset, id and length.
    slots: Vec<[AtomicU64; 2]>,
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
            slots: (0..slots).map(|_| Default::default()).collect(),
            // SAFETY: same size and alignment, and the words are only used through atomics now.
            arena: unsafe { std::slice::from_raw_parts(arena.as_ptr().cast(), arena.len()) },
            next: AtomicUsize::new(0),
            used: AtomicUsize::new(0),
        }
    }

    fn hash(name: &[u8]) -> u64 {
        words(name).fold(name.len() as u64, |state, word| {
            (state.rotate_left(5) ^ word).wrapping_mul(0x517c_c1b7_2722_0a95)
        })
    }

    fn slot(hash: u64) -> usize {
        (hash >> 20) as usize & ((1 << SLOT_BITS) - 1)
    }

    /// Starts loading the slot a lookup of `hash` probes first.
    fn prefetch(&self, hash: u64) {
        prefetch(&self.slots[Self::slot(hash)]);
    }

    /// Returns the id of `name` (whose hash is `hash`), or `None` once the table is half full.
    fn id(&self, name: &[u8], hash: u64) -> Option<usize> {
        let tag = hash | 1 << 63;
        let mask = (1 << SLOT_BITS) - 1;
        let mut slot = Self::slot(hash);
        loop {
            let mut seen = self.slots[slot][0].load(Acquire);
            if seen == 0 {
                if self.used.load(Relaxed) >= IDS || name.len() >= 1 << 16 {
                    return None;
                }
                if self.slots[slot][0]
                    .compare_exchange(0, 1, Acquire, Acquire)
                    .is_ok()
                {
                    let at = self.next.fetch_add(name.len().div_ceil(8), Relaxed);
                    for (cell, word) in self.arena[at..].iter().zip(words(name)) {
                        cell.store(word, Relaxed);
                    }
                    let id = self.used.fetch_add(1, Relaxed);
                    let span = (at as u64) << 33 | (id as u64) << 16 | name.len() as u64;
                    self.slots[slot][1].store(span, Relaxed);
                    self.slots[slot][0].store(tag, Release);
                    return Some(id);
                }
                seen = self.slots[slot][0].load(Acquire);
            }
            while seen == 1 {
                std::hint::spin_loop();
                seen = self.slots[slot][0].load(Acquire);
            }
            if seen == tag {
                let span = self.slots[slot][1].load(Relaxed);
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

/// Asks the CPU to start loading `value`'s cache line.
fn prefetch<T>(value: &T) {
    let address: *const T = value;
    #[cfg(target_arch = "x86_64")]
    // SAFETY: a prefetch never faults.
    unsafe {
        std::arch::x86_64::_mm_prefetch(address.cast(), std::arch::x86_64::_MM_HINT_T0);
    }
    #[cfg(target_arch = "aarch64")]
    // SAFETY: a prefetch never faults and touches no registers but its input.
    unsafe {
        std::arch::asm!("prfm pldl1keep, [{0}]", in(reg) address, options(nostack, readonly, preserves_flags));
    }
}

/// Records whose names are looked up together, so their cache misses overlap.
const BATCH: usize = 32;

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

/// Bit masks of the `"`, `,` and LF bytes in a 64-byte block (bit i is byte i).
#[cfg(target_arch = "x86_64")]
fn masks(block: &[u8; 64]) -> [u64; 3] {
    use std::arch::x86_64::*;
    // SAFETY: SSE2 is part of x86-64, and every load is inside the block.
    unsafe {
        let lanes = [0, 16, 32, 48].map(|at| _mm_loadu_si128(block.as_ptr().add(at).cast()));
        [b'"', b',', b'\n'].map(|byte| {
            let target = _mm_set1_epi8(byte as i8);
            lanes.iter().enumerate().fold(0, |mask, (lane, &bytes)| {
                let found = _mm_movemask_epi8(_mm_cmpeq_epi8(bytes, target)) as u16;
                mask | u64::from(found) << (16 * lane)
            })
        })
    }
}

/// Bit masks of the `"`, `,` and LF bytes in a 64-byte block (bit i is byte i).
#[cfg(target_arch = "aarch64")]
fn masks(block: &[u8; 64]) -> [u64; 3] {
    use std::arch::aarch64::*;
    // SAFETY: NEON is part of AArch64, and every load is inside the block.
    unsafe {
        let bits = vld1q_u8([1, 2, 4, 8, 16, 32, 64, 128, 1, 2, 4, 8, 16, 32, 64, 128].as_ptr());
        let lanes = [0, 16, 32, 48].map(|at| vld1q_u8(block.as_ptr().add(at)));
        [b'"', b',', b'\n'].map(|byte| {
            let [a, b, c, d] = lanes.map(|bytes| vandq_u8(vceqq_u8(bytes, vdupq_n_u8(byte)), bits));
            let sums = vpaddq_u8(vpaddq_u8(a, b), vpaddq_u8(c, d));
            vgetq_lane_u64(vreinterpretq_u64_u8(vpaddq_u8(sums, sums)), 0)
        })
    }
}

/// Decimal value of `text`'s digits; other bytes (the amount's `.`) are skipped.
fn number(text: &[u8]) -> u64 {
    text.iter()
        .filter(|byte| byte.is_ascii_digit())
        .fold(0, |value, &byte| value * 10 + u64::from(byte - b'0'))
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
///
/// Works 64 bytes at a time: a prefix XOR of the quote mask marks the bytes inside quotes (a `""`
/// pair toggles twice), and the commas and LFs outside them separate the fields. Unquoted fields
/// never contain `"` (spec 2.1), so a field with k quotes is quoted and its value is
/// `1 + k / 2` bytes shorter than its raw form.
fn records<'a>(bytes: &'a [u8], mut visit: impl FnMut(Record<'a>)) {
    let mut padded = [0; 64];
    let (mut base, mut quotes, mut inside) = (0, 0, 0_u64);
    let (mut field, mut start, mut start_quotes) = (0, 0, 0);
    let mut record: Record = (0, &[], 0, 0, 0, 0);
    // A field spans `start..end` and holds the quotes counted between `quotes.0` and `quotes.1`.
    let mut end_field = |field: usize, (start, end): (usize, usize), quotes: (usize, usize), record: &mut Record<'a>| {
        let raw = &bytes[start..end];
        let quoted = quotes.1 - quotes.0;
        let length = if quoted == 0 { raw.len() } else { raw.len() - 1 - quoted / 2 };
        match field {
            0 => record.0 = number(raw.strip_prefix(BOM).unwrap_or(raw)),
            1 => {
                record.1 = if quoted == 0 { raw } else { &raw[1..raw.len() - 1] };
                record.2 = length;
            }
            2 => record.3 = length,
            3 => record.4 = number(raw),
            _ => {
                let cents = number(raw) as i64;
                record.5 = if raw.first() == Some(&b'-') { -cents } else { cents };
                visit(*record);
            }
        }
    };
    while base < bytes.len() {
        let block: &[u8; 64] = match bytes.get(base..base + 64) {
            Some(block) => block.try_into().unwrap(),
            None => {
                padded[..bytes.len() - base].copy_from_slice(&bytes[base..]);
                &padded
            }
        };
        let [quote, comma, newline] = masks(block);
        let mut within = quote;
        for shift in [1, 2, 4, 8, 16, 32] {
            within ^= within << shift;
        }
        within ^= inside;
        inside = ((within as i64) >> 63) as u64;
        let mut separators = (comma | newline) & !within;
        while separators != 0 {
            let bit = separators.trailing_zeros();
            separators &= separators - 1;
            let end = base + bit as usize;
            let end_quotes = quotes + (quote & ((1 << bit) - 1)).count_ones() as usize;
            end_field(field, (start, end), (start_quotes, end_quotes), &mut record);
            (field, start, start_quotes) = ((field + 1) % 5, end + 1, end_quotes);
        }
        quotes += quote.count_ones() as usize;
        base += 64;
    }
    if field == 4 {
        end_field(4, (start, bytes.len()), (start_quotes, quotes), &mut record);
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
    // Group of each region id: one byte when the groups fit (the generator has 240 countries),
    // two otherwise. Holes hold the `_UNMATCHED` group.
    let regions = rows.iter().map(|&(region, _)| region + 1).max().unwrap_or(0);
    let wide = groups > 256;
    let mut narrow = vec![codes.len() as u8; if wide { 0 } else { regions }];
    let mut country = vec![codes.len() as u16; if wide { regions } else { 0 }];
    for &(region, code) in &rows {
        let group = codes.binary_search(&code).unwrap();
        match wide {
            false => narrow[region] = group as u8,
            true => country[region] = group as u16,
        }
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
    let group_of = |region: u64| match wide {
        false => narrow.get(region as usize).map(|&group| group as usize),
        true => country.get(region as usize).map(|&group| group as usize),
    }
    .unwrap_or(groups - 1);
    let words = IDS / 64;
    let names: Vec<AtomicU64> = (0..groups * words).map(|_| AtomicU64::new(0)).collect();
    let parts: Vec<Vec<Group>> = thread::scope(|scope| {
        let handles: Vec<_> = chunks(body, workers)
            .into_iter()
            .map(|chunk| {
                let (table, names, full, group_of, narrow, country) =
                    (&table, &names, &full, &group_of, &narrow, &country);
                scope.spawn(move || {
                    let mut part = vec![Group::default(); groups];
                    // A batch's records and name hashes. While a batch fills, the region lookups
                    // and the name table's slots are prefetched; the flush then reads them, and
                    // ORs into the group bitsets once their words have been prefetched too.
                    let mut batch: Vec<(Record, u64)> = Vec::with_capacity(BATCH);
                    let flush = |batch: &mut Vec<(Record, u64)>, part: &mut [Group]| {
                        let mut bits = [(0, 0); BATCH];
                        for (bit, &(record, hash)) in bits.iter_mut().zip(batch.iter()) {
                            let (id, name, name_length, note_length, region, amount) = record;
                            let index = group_of(region);
                            let group = &mut part[index];
                            group.count += 1;
                            group.id_sum += id;
                            group.amount_sum += amount;
                            group.min = group.min.min(amount);
                            group.max = group.max.max(amount);
                            group.name_bytes += name_length as u64;
                            group.note_bytes += note_length as u64;
                            match table.id(name, hash) {
                                Some(id) => {
                                    *bit = (index * words + id / 64, 1 << (id % 64));
                                    prefetch(&names[bit.0]);
                                }
                                None => full.store(true, Relaxed),
                            }
                        }
                        for &(word, bit) in &bits[..batch.len()] {
                            _ = names[word].fetch_or(bit, Relaxed);
                        }
                        batch.clear();
                    };
                    records(chunk, |record| {
                        let region = record.4 as usize;
                        match wide {
                            false => narrow.get(region).map(prefetch),
                            true => country.get(region).map(prefetch),
                        };
                        let hash = Table::hash(record.1);
                        table.prefetch(hash);
                        batch.push((record, hash));
                        if batch.len() == BATCH {
                            flush(&mut batch, &mut part);
                        }
                    });
                    flush(&mut batch, &mut part);
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
