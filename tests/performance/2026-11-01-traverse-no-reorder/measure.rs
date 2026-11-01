use std::{env, process::Command, time::Instant};

// Linux x86-64 getrusage layout, used only by this external benchmark wrapper.
#[repr(C)]
#[derive(Default)]
struct Usage {
    user_time: [i64; 2],
    system_time: [i64; 2],
    max_rss_kib: i64,
    other: [i64; 13],
}
unsafe extern "C" {
    fn wait4(pid: i32, status: *mut i32, options: i32, usage: *mut Usage) -> i32;
}
fn cgroup_peak() -> (String, &'static str) {
    for (limit, peak, label) in [
        ("/sys/fs/cgroup/memory.max", "/sys/fs/cgroup/memory.peak", "v2 memory.peak"),
        ("/sys/fs/cgroup/memory/memory.limit_in_bytes", "/sys/fs/cgroup/memory/memory.max_usage_in_bytes", "v1 max_usage"),
    ] {
        let limit = std::fs::read_to_string(limit).ok().and_then(|s| s.trim().parse::<u64>().ok());
        if limit == Some(8 * 1024 * 1024 * 1024) {
            if let Ok(value) = std::fs::read_to_string(peak) {
                if let Ok(value) = value.trim().parse::<u64>() { return (value.to_string(), label); }
            }
        }
    }
    ("null".into(), "unavailable")
}
fn main() {
    let mut args = env::args().skip(1);
    let binary = args.next().unwrap();
    let start = Instant::now();
    let child = Command::new(binary).args(args).spawn().unwrap();
    let mut usage = Usage::default();
    let mut status = 0;
    let result = unsafe { wait4(child.id() as i32, &mut status, 0, &mut usage) };
    assert_eq!(result, child.id() as i32);
    assert_eq!(status, 0);
    let seconds = start.elapsed().as_secs_f64();
    let (peak, metric) = cgroup_peak();
    println!("{{\"seconds\":{seconds},\"max_rss_kib\":{},\"cgroup_peak_bytes\":{peak},\"cgroup_metric\":\"{metric}\",\"user_s\":{},\"sys_s\":{},\"minflt\":{},\"vol_cs\":{},\"invol_cs\":{}}}", usage.max_rss_kib, usage.user_time[0] as f64 + usage.user_time[1] as f64 / 1e6, usage.system_time[0] as f64 + usage.system_time[1] as f64 / 1e6, usage.other[3], usage.other[11], usage.other[12]);
}
