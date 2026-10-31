#!/usr/bin/env python3
"""Run an isolated offline D7 full-seed1 experiment on my-vps."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
import time

root = Path(sys.argv[1]).resolve()
root.chmod(0o777)
print(f"REMOTE_ARTIFACTS={root}", flush=True)
rust = "rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb"
debian = "debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad"
python_image = "python:3.13-slim@sha256:d98cc29d3266e535dd9da1099688d9cb3ca4753813571e071763d17725460962"
data = Path("/home/student/speed-demon-2026/data/rust-validation/d/full-seed1")
reference = Path("/home/student/speed-demon-2026/results/rust-baseline-full/python-d-full-seed1/distances.txt")
expected = reference.read_bytes()
assert hashlib.sha256(expected).hexdigest() == "a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e"
(root / "cpu.txt").write_text(subprocess.check_output(["lscpu"], text=True))
(root / "host.txt").write_text(subprocess.check_output(["uname", "-a"], text=True))

def run(command, capture=False, capture_stderr=False):
    print("COMMAND=" + repr(command), flush=True)
    return subprocess.run(command, check=True, text=True, stdout=subprocess.PIPE if capture else None,
                          stderr=subprocess.PIPE if capture_stderr else None)

build = ["docker", "run", "--rm", "--network=none", "--user", f"{os.getuid()}:{os.getgid()}",
         "-e", "CARGO_HOME=/tmp/cargo", "-e", "RUSTFLAGS=-C target-cpu=native", "-v", f"{root}:/work"]
for variant in ["before", "track-d", "profile-before", "profile-after"]:
    run(build + ["-w", f"/work/{variant}", rust, "cargo", "build", "--release", "--offline", "--locked"])
run(build + ["-w", "/work", rust, "rustc", "--edition=2024", "-O", "measure.rs", "-o", "measure"])
run(build + ["-w", "/work", rust, "rustc", "--edition=2024", "--test", "-O", "tests/traverse-parser.rs", "-o", "parser-tests"])
(root / "compiler.txt").write_text(run(build + [rust, "rustc", "-Vv"], capture=True).stdout)
common = ["docker", "run", "--rm", "--network=none", "--read-only", "--cpuset-cpus=0-5", "--memory=8g",
          "--memory-swap=8g", "--pids-limit=512", "--cap-drop=ALL", "--security-opt=no-new-privileges",
          "--user", "65534:65534", "--tmpfs", "/tmp:rw,size=1g", "-v", f"{root}:/work"]
(root / "regression-tests.txt").write_text(run(common + [debian, "/work/parser-tests"], capture=True).stdout)
(root / "python-checks.txt").write_text(run(common + [python_image, "python3", "/work/tests/traverse-check.py",
                                                                 "/work/track-d/target/release/traverse-baseline"], capture=True).stdout)
print("REGRESSION_TESTS=" + (root / "regression-tests.txt").read_text(), flush=True)
print("PYTHON_CHECKS=" + (root / "python-checks.txt").read_text(), flush=True)
variants = {f"{label}-{n}": [f"/work/{project}/target/release/traverse-baseline", "--threads", str(n)]
            for label, project in [("before", "before"), ("after", "track-d")] for n in [1, 6, 12]}
rows = []
names = list(variants)
with (root / "runs.jsonl").open("w") as log:
    for round_number in range(6):
        order = names[round_number:] + names[:round_number]
        for variant in order:
            # Warm every input before creating the measured container.
            for path in data.iterdir():
                with path.open("rb") as stream:
                    while stream.read(1024 * 1024):
                        pass
            output = f"/work/out-{variant}"
            command = common + ["-v", f"{data}:/data:ro", debian, "/work/measure"] + variants[variant] + ["--data", "/data", "--out", output]
            completed = run(command, capture=True)
            row = json.loads(completed.stdout)
            assert (root / f"out-{variant}/distances.txt").read_bytes() == expected
            row.update(round=round_number, variant=variant, python_byte_equal=True)
            rows.append(row)
            line = json.dumps(row)
            print("RESULT=" + line, flush=True)
            log.write(line + "\n")
            log.flush()
summary = {"source_metadata": json.loads((root / "source-metadata.json").read_text()),
           "cpuset": "0-5", "memory_limit": "8g", "output_sha256": hashlib.sha256(expected).hexdigest(),
           "method": "full-seed1; prewarmed inputs; one warmup, five rotating-order runs; trimmed mean of middle three",
           "memory_metric": "child ru_maxrss from wait4, KiB; not cgroup memory.peak", "rows": rows, "variants": {}}
for name in names:
    selected = [row for row in rows if row["round"] > 0 and row["variant"] == name]
    seconds = sorted(row["seconds"] for row in selected)
    summary["variants"][name] = {"trimmed_mean_seconds": statistics.mean(seconds[1:-1]),
                                  "median_seconds": statistics.median(seconds),
                                  "spread_percent": 100 * (max(seconds) / min(seconds) - 1),
                                  "max_rss_kib": max(row["max_rss_kib"] for row in selected)}
base = summary["variants"]["before-6"]["trimmed_mean_seconds"]
for value in summary["variants"].values():
    value["speedup_vs_before_6"] = base / value["trimmed_mean_seconds"]
summary["paired_comparison"] = {}
for workers in [1, 6, 12]:
    before = summary["variants"][f"before-{workers}"]
    after = summary["variants"][f"after-{workers}"]
    summary["paired_comparison"][str(workers)] = {
        "time_change_percent": 100 * (after["trimmed_mean_seconds"] / before["trimmed_mean_seconds"] - 1),
        "max_rss_change_percent": 100 * (after["max_rss_kib"] / before["max_rss_kib"] - 1)}
(root / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
print("SUMMARY=" + json.dumps(summary["variants"]), flush=True)
profiles = []
with (root / "profiles.jsonl").open("w") as log:
    for round_number in range(3):
        for variant in (["before", "after"] if round_number % 2 == 0 else ["after", "before"]):
            for path in data.iterdir():
                with path.open("rb") as stream:
                    while stream.read(1024 * 1024):
                        pass
            command = common + ["-v", f"{data}:/data:ro", debian, "/work/measure",
                                f"/work/profile-{variant}/target/release/traverse-baseline",
                                "--threads", "6", "--data", "/data", "--out", f"/work/profile-out-{variant}"]
            completed = run(command, capture=True, capture_stderr=True)
            assert (root / f"profile-out-{variant}/distances.txt").read_bytes() == expected
            row = {"variant": variant, "round": round_number, "measurement": json.loads(completed.stdout),
                   "python_byte_equal": True, "stderr": completed.stderr}
            for line in completed.stderr.splitlines():
                if line.startswith("PHASES="):
                    row["phases"] = json.loads(line.removeprefix("PHASES="))
                elif line.startswith("GRAPH_PHASES="):
                    row["graph_phases"] = json.loads(line.removeprefix("GRAPH_PHASES="))
            assert "phases" in row and "graph_phases" in row
            profiles.append(row)
            log.write(json.dumps(row) + "\n")
            log.flush()
            print("PROFILE=" + json.dumps(row), flush=True)
profile_summary = {}
for variant in ["before", "after"]:
    selected = [row for row in profiles if row["variant"] == variant]
    profile_summary[variant] = {key: {name: statistics.median(row[key][name] for row in selected)
                                     for name in selected[0][key]} for key in ["phases", "graph_phases"]}
(root / "profile-summary.json").write_text(json.dumps(profile_summary, indent=2) + "\n")
print("PROFILE_SUMMARY=" + json.dumps(profile_summary), flush=True)
print(f"DONE={root}", flush=True)
