use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, File};
use std::hash::{BuildHasherDefault, Hasher};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::thread;

const BOM: &[u8] = b"\xEF\xBB\xBF";

/// Word-at-a-time multiplicative hash for name bytes (keys are compared in full).
#[derive(Default)]
struct WordHasher(u64);

impl Hasher for WordHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for chunk in &mut chunks {
            let word = u64::from_le_bytes(chunk.try_into().unwrap());
            self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7cc1_b727_220a_95);
        }
        for &byte in chunks.remainder() {
            self.0 = (self.0.rotate_left(5) ^ u64::from(byte)).wrapping_mul(0x51_7cc1_b727_220a_95);
        }
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

type Names<'a> = HashMap<Cow<'a, [u8]>, u32, BuildHasherDefault<WordHasher>>;

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

/// One chunk's aggregates, its own name table, and per group the local ids of its names.
struct Part<'a> {
    groups: Vec<Group>,
    names: Names<'a>,
    seen: Vec<Vec<u32>>,
}

/// Reads one field at `at`; returns its value, its unescaped length and the index after the
/// separator that follows it. A quoted field ends at a `"` not followed by another `"`.
fn field(bytes: &[u8], at: usize) -> (Cow<'_, [u8]>, usize, usize) {
    if bytes.get(at) != Some(&b'"') {
        let end = at + bytes[at..].iter().position(|&byte| byte == b',').unwrap();
        return (Cow::Borrowed(&bytes[at..end]), end - at, end + 1);
    }
    let (start, mut from, mut escapes) = (at + 1, at + 1, 0);
    let end = loop {
        let quote = from + bytes[from..].iter().position(|&byte| byte == b'"').unwrap();
        if bytes.get(quote + 1) == Some(&b'"') {
            escapes += 1;
            from = quote + 2;
        } else {
            break quote;
        }
    };
    let raw = &bytes[start..end];
    let value = if escapes == 0 {
        Cow::Borrowed(raw)
    } else {
        let mut owned = Vec::with_capacity(raw.len() - escapes);
        let mut quote = false;
        for &byte in raw {
            if byte == b'"' && quote {
                quote = false;
                continue;
            }
            quote = byte == b'"';
            owned.push(byte);
        }
        Cow::Owned(owned)
    };
    (value, raw.len() - escapes, end + 2)
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

/// Aggregates every record in `bytes`, which starts at a record and ends after a terminator
/// (or at the end of the file). `country[region]` is a group, or `unmatched`.
fn parse<'a>(bytes: &'a [u8], country: &[u16], groups: usize) -> Part<'a> {
    let unmatched = groups - 1;
    let mut part = Part {
        groups: vec![Group::default(); groups],
        names: Names::default(),
        seen: vec![Vec::new(); groups],
    };
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
        let amount = if negative { -cents } else { cents };
        at = next + 2;
        at += usize::from(bytes.get(at) == Some(&b'\r'));
        at += usize::from(bytes.get(at) == Some(&b'\n'));
        let index = country
            .get(region as usize)
            .map_or(unmatched, |&group| group as usize);
        let group = &mut part.groups[index];
        group.count += 1;
        group.id_sum += id;
        group.amount_sum += amount;
        group.min = group.min.min(amount);
        group.max = group.max.max(amount);
        group.name_bytes += name_length as u64;
        group.note_bytes += note_length as u64;
        let next_id = part.names.len() as u32;
        let local = *part.names.entry(name).or_insert(next_id);
        part.seen[index].push(local);
    }
    part
}

/// Splits `bytes` (records only) into up to `pieces` chunks that each start at a record. Under
/// the generator's guarantees (spec 2.2) a line inside a quoted note never starts with a digit
/// or a BOM, so an LF followed by either is always a record terminator.
fn chunks(bytes: &[u8], pieces: usize) -> Vec<&[u8]> {
    // A record start: an optional BOM, then digits and a comma.
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
    // Regions: `region_id,country` lines after the header, ids ascending.
    let regions = map_file(&data.join("regions.csv"))?;
    let mut rows = Vec::new();
    for line in regions
        .split(|&byte| byte == b'\n')
        .skip(1)
        .filter(|line| !line.is_empty())
    {
        let comma = line
            .iter()
            .position(|&byte| byte == b',')
            .ok_or("bad region line")?;
        let (region, _) = digits(line, 0, b',');
        rows.push((region as usize, &line[comma + 1..]));
    }
    let mut codes: Vec<&[u8]> = rows.iter().map(|&(_, code)| code).collect();
    codes.sort_unstable();
    codes.dedup();
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
    let records = map_file(&data.join("records.csv"))?;
    let records = records.strip_prefix(BOM).unwrap_or(records);
    let body = &records[records
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(records.len(), |at| at + 1)..];
    let country = &country;
    let parts: Vec<Part> = thread::scope(|scope| {
        let handles: Vec<_> = chunks(body, workers)
            .into_iter()
            .map(|chunk| scope.spawn(move || parse(chunk, country, groups)))
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    });
    // Merge: one global id per distinct name, then one bitset of names per group.
    let mut global = Names::default();
    let mut translate = Vec::with_capacity(parts.len());
    for part in &parts {
        let mut map = vec![0_u32; part.names.len()];
        for (name, &local) in &part.names {
            let next = global.len() as u32;
            map[local as usize] = *global.entry(Cow::Borrowed(name.as_ref())).or_insert(next);
        }
        translate.push(map);
    }
    let words = global.len().div_ceil(64);
    let mut text = Vec::new();
    let mut bits = vec![0_u64; words];
    for index in 0..groups {
        let mut total = Group::default();
        bits.fill(0);
        for (part, map) in parts.iter().zip(&translate) {
            let group = &part.groups[index];
            total.count += group.count;
            total.id_sum += group.id_sum;
            total.amount_sum += group.amount_sum;
            total.min = total.min.min(group.min);
            total.max = total.max.max(group.max);
            total.name_bytes += group.name_bytes;
            total.note_bytes += group.note_bytes;
            for &local in &part.seen[index] {
                let id = map[local as usize] as usize;
                bits[id / 64] |= 1 << (id % 64);
            }
        }
        if total.count == 0 {
            continue;
        }
        let distinct: u32 = bits.iter().map(|word| word.count_ones()).sum();
        text.extend_from_slice(codes.get(index).copied().unwrap_or(b"_UNMATCHED"));
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
