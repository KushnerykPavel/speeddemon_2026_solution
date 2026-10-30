#!/usr/bin/env python3
"""Native Track A reference comparison and whole-process benchmark (not the board)."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import time


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def execute(binary, data, output):
    output.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    process = subprocess.Popen([str(binary), "--data", str(data), "--out", str(output)])
    _, status, usage = os.wait4(process.pid, 0)
    elapsed = time.perf_counter() - started
    process.returncode = os.waitstatus_to_exitcode(status)
    if process.returncode:
        raise subprocess.CalledProcessError(process.returncode, process.args)
    return {
        "seconds": elapsed,
        "peak_rss_bytes": usage.ru_maxrss * (1 if sys.platform == "darwin" else 1024),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--work", type=Path, required=True)
    parser.add_argument("--summary", type=Path, required=True)
    parser.add_argument("--reference-work", type=Path,
                        help="reuse unchanged Python outputs from a previous run")
    parser.add_argument("--reference-summary", type=Path,
                        help="verify reused reference source, inputs, and output hashes")
    args = parser.parse_args()
    if bool(args.reference_work) != bool(args.reference_summary):
        parser.error("--reference-work and --reference-summary must be supplied together")
    args.work.mkdir(parents=True, exist_ok=False)
    result = {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "logical_cpus": os.cpu_count(),
        "python": sys.version,
        "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
        "conditions": {
            "native": True, "docker": False, "cpu_affinity": None,
            "warmups_per_variant": 1, "measured_runs_per_variant": 5,
            "order": "alternating before/after each round",
            "memory": "per-child peak RSS from wait4; not cgroup memory.peak",
            "timing": "process spawn through exit; output comparison excluded",
            "cache": "warm input; reference and correctness runs precede benchmark",
        },
        "source_sha256": {
            "before": sha256(Path(__file__).with_name("before-main.rs")),
            "after": sha256(args.repo / "track-a/src/main.rs"),
            "manifest": sha256(args.repo / "track-a/Cargo.toml"),
            "reference": sha256(args.repo / "tests/reference/a_ingest.py"),
        },
        "binary_sha256": {"before": sha256(args.before), "after": sha256(args.after)},
        "correctness": [],
        "benchmark": [],
    }
    cached_cases = {}
    if args.reference_summary:
        cached = json.loads(args.reference_summary.read_text())
        assert cached["source_sha256"]["reference"] == result["source_sha256"]["reference"]
        cached_cases = {case["case"]: case for case in cached["correctness"]}
        result["reference_reused_from"] = str(args.reference_summary)
    published = {
        "sample-seed1": "ca6311bbeb536fa55d85265bf6fd3d24cc9586f13de1ac73bd2a23ac3893aac3",
        "full-seed1": "495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3",
    }
    names = ["empty", "edge-cases"] + [
        f"{scale}-seed{seed}" for scale in ["tiny", "sample"] for seed in [1, 7, 42]
    ] + ["full-seed1"]
    for name in names:
        data = args.data_root / name
        reference = (args.reference_work or args.work) / name / "reference"
        input_hash = sha256(data / "requests.log")
        if not args.reference_work:
            subprocess.run([
                sys.executable, str(args.repo / "tests/reference/a_ingest.py"),
                "--data", str(data), "--out", str(reference),
            ], check=True)
        expected = (reference / "summary.txt").read_bytes()
        expected_hash = hashlib.sha256(expected).hexdigest()
        if args.reference_work:
            assert cached_cases[name]["input_sha256"] == input_hash, name
            assert cached_cases[name]["output_sha256"] == expected_hash, name
        if name in published:
            assert expected_hash == published[name], (name, "published hash mismatch")
        for variant in ["before", "after"]:
            output = args.work / name / variant
            execute(getattr(args, variant), data, output)
            assert (output / "summary.txt").read_bytes() == expected, (name, variant)
        result["correctness"].append({
            "case": name, "input_sha256": input_hash,
            "output_sha256": expected_hash, "byte_equal": True,
            "published_hash_checked": name in published,
        })
        print(f"PASS {name}: Python == before == after ({expected_hash})", flush=True)
    data = args.data_root / "full-seed1"
    expected = ((args.reference_work or args.work) / "full-seed1/reference/summary.txt").read_bytes()
    for round_number in range(6):
        order = ["before", "after"] if round_number % 2 == 0 else ["after", "before"]
        for variant in order:
            output = args.work / "benchmark" / variant
            measurement = execute(getattr(args, variant), data, output)
            assert (output / "summary.txt").read_bytes() == expected, variant
            measurement.update(variant=variant, round=round_number, warmup=round_number == 0)
            result["benchmark"].append(measurement)
            print(json.dumps(measurement), flush=True)
    result["trimmed_mean_seconds"] = {}
    for variant in ["before", "after"]:
        times = sorted(row["seconds"] for row in result["benchmark"]
                       if row["variant"] == variant and not row["warmup"])
        result["trimmed_mean_seconds"][variant] = statistics.mean(times[1:-1])
    before, after = (result["trimmed_mean_seconds"][v] for v in ["before", "after"])
    result["speedup"] = before / after
    result["time_reduction_percent"] = 100 * (1 - after / before)
    args.summary.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({key: result[key] for key in [
        "trimmed_mean_seconds", "speedup", "time_reduction_percent"
    ]}), flush=True)


if __name__ == "__main__":
    main()
