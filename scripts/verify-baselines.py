#!/usr/bin/env python3
"""Compare the Rust containers byte-for-byte with the preserved Python references.

Requires Docker and a checkout of the official generator repository. Inputs,
reference outputs and per-case logs are retained outside the track folders.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
PYTHON_IMAGE = "python:3.13-slim@sha256:d98cc29d3266e535dd9da1099688d9cb3ca4753813571e071763d17725460962"
MODULES = {"a": "a_ingest", "d": "d_traverse"}
OUTPUTS = {"a": "summary.txt", "d": "distances.txt"}
EXPECTED = {
    ("a", "sample"): "ca6311bbeb536fa55d85265bf6fd3d24cc9586f13de1ac73bd2a23ac3893aac3",
    ("a", "full"): "495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3",
    ("d", "sample"): "a4c76ed030bc7e365db33191f6cf74708fba7f25eb48d64c8c4279f2e57046a0",
    ("d", "full"): "a23dbdf0a3e49cc8901310a9ee1dc6a9a569a85297b6da75c2a7002cb2ed277e",
}


def execute(command, log):
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    log.write(result.stdout)
    log.flush()
    if result.returncode:
        print(result.stdout, flush=True)
        raise subprocess.CalledProcessError(result.returncode, command)
    return result.stdout


def fixtures(root):
    """Exercise percentile boundaries, date/order quirks and shortest-path edges."""
    cases = []
    for name in ("edge-cases", "empty"):
        data = root / "a" / name
        data.mkdir(parents=True, exist_ok=True)
        rows = []
        if name == "edge-cases":
            for count in range(1, 103):
                for i in range(count):
                    stamp = ("2026-11-01T00:00:00.000000Z" if count % 2 else
                             "2026-10-31T23:59:59.999999Z")
                    status = (499, 500, 599)[i % 3]
                    latency = (0, 4294967295, i)[i % 3]
                    rows.append(f"{stamp} GET /group/{count} {status} {latency}\n")
            rows.extend(["2026-10-30T18:00:00.000000Z OPTIONS / 599 4294967295\n"] * 2)
            rows.append("2026-11-02T17:59:59.999999Z GET /" + "x" * 124 + " 100 0\n")
            random.Random(17).shuffle(rows)
        (data / "requests.log").write_text("".join(rows), encoding="ascii")
        cases.append(("a", name, data, None))
    data = root / "d" / "edge-cases"
    data.mkdir(parents=True, exist_ok=True)
    (data / "graph.txt").write_text(
        "8 12\n0 1 4\n1 2 3\n0 2 9\n2 0 5\n2 1 6\n1 3 0\n3 4 0\n"
        "4 4 7\n4 5 4294967295\n2 1 2\n5 6 4294967295\n0 1 4\n", encoding="ascii")
    (data / "queries.txt").write_text(
        "0 2\n2 1\n1 4\n4 4\n7 7\n0 6\n4 5\n6 0\n0 7\n1 0\n2 1\n", encoding="ascii")
    cases.append(("d", "edge-cases", data,
                  "2e9871bba1d5013144ceff48d806a28da0e3f203b29118a89d98d8f654c3f6a0"))
    data = root / "d" / "no-edges"
    data.mkdir(parents=True, exist_ok=True)
    (data / "graph.txt").write_text("3 0\n", encoding="ascii")
    (data / "queries.txt").write_text("0 0\n0 1\n2 2\n", encoding="ascii")
    cases.append(("d", "no-edges", data, None))
    data = root / "d" / "no-queries"
    data.mkdir(parents=True, exist_ok=True)
    (data / "graph.txt").write_text("1 0\n", encoding="ascii")
    (data / "queries.txt").write_text("", encoding="ascii")
    cases.append(("d", "no-queries", data, None))
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--harness", required=True, type=Path)
    parser.add_argument("--data-root", required=True, type=Path)
    parser.add_argument("--results", required=True, type=Path,
                        help="new directory, to preserve previous run evidence")
    parser.add_argument("--scales", nargs="+", choices=("tiny", "sample", "full"), default=["tiny", "sample"])
    parser.add_argument("--seeds", nargs="+", type=int, default=[1, 7, 42])
    args = parser.parse_args()
    harness, data_root, results = args.harness.resolve(), args.data_root.resolve(), args.results.resolve()
    data_root.mkdir(parents=True, exist_ok=True)
    results.mkdir(parents=True, exist_ok=False)
    cases = fixtures(data_root)
    summary = []
    built = set()
    for scale in args.scales:
        for seed in args.seeds:
            for track, module in MODULES.items():
                data = data_root / track / f"{scale}-seed{seed}"
                data.mkdir(parents=True, exist_ok=True)
                name = f"{scale}-seed{seed}"
                with (results / f"generate-{track}-{name}.log").open("w") as log:
                    execute(["docker", "run", "--rm", "--network=none", "--memory=8g", "--memory-swap=8g",
                             "--user", f"{os.getuid()}:{os.getgid()}",
                             "-v", f"{harness}:/harness:ro", "-v", f"{data}:/data:rw", PYTHON_IMAGE,
                             "python3", f"/harness/generators/{module}.py", "--seed", str(seed),
                             "--scale", scale, "--out", "/data"], log)
                cases.append((track, name, data, EXPECTED.get((track, scale)) if seed == 1 else None))
                print(f"GENERATED {track.upper()} {name}", flush=True)
    for track, name, data, published in cases:
        output = results / f"python-{track}-{name}"
        output.mkdir(mode=0o777)
        output.chmod(0o777)
        with (results / f"check-{track}-{name}.log").open("w") as log:
            execute(["docker", "run", "--rm", "--network=none", "--read-only",
                     "--memory=8g", "--memory-swap=8g", "--pids-limit=512",
                     "--cap-drop=ALL", "--security-opt=no-new-privileges", "--user", "65534:65534",
                     "-v", f"{ROOT / 'tests/reference'}:/reference:ro", "-v", f"{data}:/data:ro",
                     "-v", f"{output}:/out:rw", PYTHON_IMAGE, "python3",
                     f"/reference/{MODULES[track]}.py"], log)
            reference = (output / OUTPUTS[track]).read_bytes()
            expected = hashlib.sha256(reference).hexdigest()
            if published and expected != published:
                raise RuntimeError(f"Python reference differs from published hash: {track} {name}")
            command = [sys.executable, str(ROOT / "check.py"), track, "--data", str(data), "--expected", expected]
            if track in built:
                command.append("--skip-build")
            text = execute(command, log)
            built.add(track)
            actual = (ROOT / ".out" / f"track-{track}" / OUTPUTS[track]).read_bytes()
            if actual != reference:
                raise RuntimeError(f"Byte mismatch: {track} {name}")
            elapsed = re.search(r"time  ([\d.]+) s", text)
            summary.append({"track": track, "case": name, "sha256": expected,
                            "rust_seconds_including_container_start": float(elapsed[1]),
                            "byte_equal": True, "published_hash_checked": published is not None})
            (results / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
        print(f"PASS {track.upper()} {name}: byte-identical ({elapsed[1]} s with container startup)", flush=True)
    print(f"PASS: {len(summary)} comparisons. Results: {results}", flush=True)


if __name__ == "__main__":
    main()
