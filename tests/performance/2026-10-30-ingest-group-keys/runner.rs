
use std::{fs,process::Command,time::Instant};
fn main() {
    let mut reference = None;
    for round in 0..6 {
        let variants = if round % 2 == 0 { ["before", "after"] } else { ["after", "before"] };
        for variant in variants {
            let output = format!("/work/out-{variant}");
            let start = Instant::now();
            let status = Command::new(format!("/work/{variant}/target/release/ingest-baseline"))
                .args(["--data", "/data", "--out", &output]).status().unwrap();
            let seconds = start.elapsed().as_secs_f64();
            assert!(status.success());
            let bytes = fs::read(format!("{output}/summary.txt")).unwrap();
            if let Some(expected) = &reference { assert_eq!(&bytes, expected); } else { reference = Some(bytes); }
            println!("{{\"round\":{round},\"variant\":\"{variant}\",\"seconds\":{seconds},\"byte_equal\":true}}");
        }
    }
}
