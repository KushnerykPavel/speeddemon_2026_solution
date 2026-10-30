"""Read JSON source bundle on stdin; build and measure in a fresh VPS directory."""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile

bundle = json.load(sys.stdin)
work = Path(tempfile.mkdtemp(prefix="ingest-byte-linux-", dir="/tmp"))
print("REMOTE_DIR", work, flush=True)
for name, contents in bundle["files"].items():
    target = work / name
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(contents)
image = "rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb"
base = ["docker", "run", "--rm", "--network=none", "-v", f"{work}:/work", "-w", "/work", image]
for name in ("before", "track-a"):
    subprocess.run(base + ["cargo", "build", "--manifest-path", f"{name}/Cargo.toml",
                          "--release", "--offline", "--locked"], check=True)
compiler = subprocess.check_output(base + ["rustc", "-Vv"], text=True)
(work / "rustc-version.txt").write_text(compiler)
subprocess.run(base + ["rustc", "--edition=2024", "--test", "-O", "tests/ingest-parser.rs",
                      "-o", "parser-tests"], check=True)
tests = subprocess.check_output([str(work / "parser-tests")], text=True)
print(tests, flush=True)
(work / "regression-tests.txt").write_text(tests)
(work / "cpu.txt").write_text(subprocess.check_output(["lscpu"], text=True))
metadata = {"remote_directory": str(work), "repo_commit": bundle["commit"],
            "kernel": platform.platform(), "build_image": image,
            "build": "cargo build --release --offline --locked", "runtime": "native Linux host",
            "load_before": os.getloadavg(), "cpu_affinity": [2],
            "source_sha256": {name: hashlib.sha256(contents.encode()).hexdigest()
                              for name, contents in bundle["files"].items()}}
runner = work / "runner.py"
data = "/home/student/speed-demon-2026/data/rust-validation/a"
for batch in (1, 2):
    command = ["taskset", "-c", "2", "python3", str(runner), "--repo", str(work),
               "--data-root", data, "--before", str(work / "before/target/release/ingest-baseline"),
               "--after", str(work / "track-a/target/release/ingest-baseline"),
               "--work", str(work / f"batch{batch}"), "--summary", str(work / f"summary{batch}.json"),
               "--rustc-version-file", str(work / "rustc-version.txt")]
    if batch == 2:
        command += ["--reference-work", str(work / "batch1"),
                    "--reference-summary", str(work / "summary1.json")]
    metadata.setdefault("commands", []).append(command)
    print("START_BATCH", batch, flush=True)
    process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    with (work / f"raw{batch}.txt").open("w") as log:
        for line in process.stdout:
            log.write(line)
            print(line, end="", flush=True)
    if process.wait():
        raise RuntimeError(f"batch {batch} failed; artifacts retained in {work}")
    summary = json.loads((work / f"summary{batch}.json").read_text())
    spread = {}
    for variant in ("before", "after"):
        values = [row["seconds"] for row in summary["benchmark"]
                  if row["variant"] == variant and not row["warmup"]]
        spread[variant] = max(values) / min(values) - 1
    print("SPREAD", batch, json.dumps(spread), flush=True)
    if batch == 1 and max(spread.values()) <= 0.05:
        break
metadata["load_after"] = os.getloadavg()
(work / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
files = {p.name: p.read_text() for p in work.iterdir()
         if p.is_file() and (p.name.startswith(("raw", "summary")) or
                            p.name in ("metadata.json", "cpu.txt", "rustc-version.txt", "regression-tests.txt"))}
print("__ARTIFACTS__" + json.dumps(files), flush=True)
