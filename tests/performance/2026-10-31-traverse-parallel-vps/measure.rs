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
    println!("{{\"seconds\":{},\"max_rss_kib\":{}}}", start.elapsed().as_secs_f64(), usage.max_rss_kib);
}
