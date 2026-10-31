use std::time::Instant;
unsafe extern "C" {
    fn prctl(option: i32, a: u64, b: u64, c: u64, d: u64) -> i32;
    fn mallopt(p: i32, v: i32) -> i32;
}
fn main() {
    unsafe {
        prctl(41, 1, 0, 0, 0);
        mallopt(-3, 256 << 10);
    }
    let n = 7_000_000usize;
    for round in 0..3 {
        let t = Instant::now();
        let mut v = vec![0u32; n];
        for i in (0..n).step_by(1024) {
            v[i] = i as u32;
        }
        let first = t.elapsed().as_secs_f64();
        let t = Instant::now();
        for i in (0..n).step_by(1024) {
            v[i] += 1;
        }
        let second = t.elapsed().as_secs_f64();
        let t = Instant::now();
        let mut w = vec![0u32; n];
        let base = w.as_mut_ptr() as usize;
        let chunk = n / 6 + 1;
        std::thread::scope(|s| {
            for k in 0..6 {
                s.spawn(move || {
                    let p = base as *mut u32;
                    for i in (k * chunk..((k + 1) * chunk).min(n)).step_by(1024) {
                        unsafe { *p.add(i) = 1 }
                    }
                });
            }
        });
        let par = t.elapsed().as_secs_f64();
        println!("round {round}: touch 28MB serial first {:.1} ms, again {:.2} ms, 6-thread first {:.1} ms (sum {})", first * 1e3, second * 1e3, par * 1e3, v[1024] + w[0]);
    }
}
