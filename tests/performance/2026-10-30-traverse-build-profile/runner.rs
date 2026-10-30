
use std::{fs,process::Command,time::Instant};
fn main() {
    let reference = fs::read("/work/reference.txt").unwrap();
    for round in 0..6 {
        let variants = if round % 2 == 0 { ["before", "after"] } else { ["after", "before"] };
        for variant in variants {
            let output = format!("/work/out-{variant}");
            let start = Instant::now();
            let status = Command::new(format!("/work/{variant}/target/release/traverse-baseline"))
                .args(["--data", "/data", "--out", &output]).status().unwrap();
            let seconds = start.elapsed().as_secs_f64();
            assert!(status.success());
            let bytes = fs::read(format!("{output}/distances.txt")).unwrap();
            assert_eq!(bytes, reference);
            println!("{{\"round\":{round},\"variant\":\"{variant}\",\"seconds\":{seconds},\"byte_equal\":true}}");
        }
    }
}
