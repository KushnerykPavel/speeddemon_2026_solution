#!/usr/bin/env python3
"""Diagnostic local A/B benchmark for degree rejection; not an official workload."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import random
import runpy
import statistics
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("before", type=Path)
    parser.add_argument("after", type=Path)
    args = parser.parse_args()
    binaries = {key: path.resolve() for key, path in vars(args).items()}
    reference = runpy.run_path(str(Path(__file__).resolve().parents[2] / "reference/d_traverse.py"))
    rng = random.Random(20261031)
    width = 300
    body = width * width
    ids = list(range(body + 3))
    rng.shuffle(ids)
    no_in, no_out, isolated = ids[body:]
    edges = []
    for node in range(body):
        if node % width + 1 < width:
            edges.extend([(ids[node], ids[node + 1], 1), (ids[node + 1], ids[node], 1)])
        if node + width < body:
            edges.extend([(ids[node], ids[node + width], 1), (ids[node + width], ids[node], 1)])
    edges.extend([(no_in, ids[0], 0), (ids[-4], no_out, 0)])
    rng.shuffle(edges)
    normal = []
    for _ in range(1950):
        row, col = rng.randrange(width), rng.randrange(width)
        target_row = min(width - 1, max(0, row + rng.randrange(-16, 17)))
        target_col = min(width - 1, max(0, col + rng.randrange(-16, 17)))
        normal.append((ids[row * width + col], ids[target_row * width + target_col]))
    special = [(ids[0], no_in), (ids[body // 2], no_in),
               (no_out, ids[0]), (no_out, ids[body // 2]),
               (isolated, ids[0]), (isolated, ids[body // 2]),
               (isolated, isolated), (no_in, no_in), (no_out, no_out)]
    special += [(ids[i], ids[i]) for i in range(41)]
    workloads = {"normal_only": normal, "mixed": normal + special}
    results = {}
    with tempfile.TemporaryDirectory(prefix="traverse-degree-") as temporary:
        root = Path(temporary)
        (root / "expected").mkdir()
        (root / "graph.txt").write_text(f"{len(ids)} {len(edges)}\n" + "".join(f"{u} {v} {w}\n" for u, v, w in edges))
        graph_hash = hashlib.sha256((root / "graph.txt").read_bytes()).hexdigest()
        for name, queries in workloads.items():
            (root / "queries.txt").write_text("".join(f"{s} {t}\n" for s, t in queries))
            reference["main"](str(root), str(root / "expected"))
            expected = (root / "expected/distances.txt").read_bytes()
            rows = []
            for round_number in range(6):
                for variant in (["before", "after"] if round_number % 2 == 0 else ["after", "before"]):
                    start = time.perf_counter()
                    subprocess.run([str(binaries[variant]), "--data", str(root), "--out", str(root / variant)], check=True)
                    seconds = time.perf_counter() - start
                    assert (root / variant / "distances.txt").read_bytes() == expected
                    rows.append({"round": round_number, "variant": variant, "seconds": seconds, "python_byte_equal": True})
            means = {}
            for variant in binaries:
                timings = sorted(row["seconds"] for row in rows if row["round"] > 0 and row["variant"] == variant)
                means[variant] = statistics.mean(timings[1:-1])
            results[name] = {"queries": len(queries), "rows": rows, "trimmed_mean_seconds": means,
                             "time_reduction_percent": 100 * (1 - means["after"] / means["before"])}
    print(json.dumps({"platform": platform.platform(), "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
                      "seed": 20261031, "nodes": len(ids), "edges": len(edges), "graph_sha256": graph_hash,
                      "method": "warm cache; one warmup, five alternating runs; discard min/max, average remaining three",
                      "results": results}, indent=2))


if __name__ == "__main__":
    main()
