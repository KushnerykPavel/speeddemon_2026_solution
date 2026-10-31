#!/usr/bin/env python3
"""Local D7 thread sweep; synthetic diagnostic, not an official workload."""
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
    before, after = args.before.resolve(), args.after.resolve()
    rng = random.Random(20261031)
    width = 300
    nodes = width * width
    ids = list(range(nodes))
    rng.shuffle(ids)
    edges = []
    for node in range(nodes):
        if node % width + 1 < width:
            edges.extend([(ids[node], ids[node + 1], 1), (ids[node + 1], ids[node], 1)])
        if node + width < nodes:
            edges.extend([(ids[node], ids[node + width], 1), (ids[node + width], ids[node], 1)])
    rng.shuffle(edges)
    queries = []
    for index in range(1000):
        row, col = rng.randrange(width), rng.randrange(width)
        radius = 1 << rng.randrange(7)
        target_row = min(width - 1, max(0, row + rng.randrange(-radius, radius + 1)))
        target_col = min(width - 1, max(0, col + rng.randrange(-radius, radius + 1)))
        if index % 17 == 0:
            target_row, target_col = width - 1 - row, width - 1 - col
        queries.append((ids[row * width + col], ids[target_row * width + target_col]))
    variants = {"before": [str(before)], **{str(workers): [str(after), "--threads", str(workers)] for workers in [1, 2, 4, 6, 12]}}
    rows = []
    reference = runpy.run_path(str(Path(__file__).resolve().parents[2] / "reference/d_traverse.py"))
    with tempfile.TemporaryDirectory(prefix="traverse-parallel-") as temporary:
        root = Path(temporary)
        (root / "expected").mkdir()
        (root / "graph.txt").write_text(f"{nodes} {len(edges)}\n" + "".join(f"{u} {v} {w}\n" for u, v, w in edges))
        (root / "queries.txt").write_text("".join(f"{source} {target}\n" for source, target in queries))
        graph_hash = hashlib.sha256((root / "graph.txt").read_bytes()).hexdigest()
        query_hash = hashlib.sha256((root / "queries.txt").read_bytes()).hexdigest()
        reference["main"](str(root), str(root / "expected"))
        expected = (root / "expected/distances.txt").read_bytes()
        names = list(variants)
        for round_number in range(6):
            order = names[round_number:] + names[:round_number]
            for variant in order:
                start = time.perf_counter()
                subprocess.run(variants[variant] + ["--data", str(root), "--out", str(root / variant)], check=True)
                seconds = time.perf_counter() - start
                assert (root / variant / "distances.txt").read_bytes() == expected
                rows.append({"round": round_number, "variant": variant, "seconds": seconds, "python_byte_equal": True})
        means = {}
        for variant in variants:
            timings = sorted(row["seconds"] for row in rows if row["round"] > 0 and row["variant"] == variant)
            means[variant] = statistics.mean(timings[1:-1])
    print(json.dumps({"platform": platform.platform(), "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
                      "seed": 20261031, "nodes": nodes, "edges": len(edges), "queries": len(queries),
                      "graph_sha256": graph_hash, "queries_sha256": query_hash,
                      "method": "warm cache; one warmup, five rotating-order runs; discard min/max and average middle three",
                      "rows": rows, "trimmed_mean_seconds": means,
                      "speedup_vs_before": {key: means["before"] / value for key, value in means.items()}}, indent=2))


if __name__ == "__main__":
    main()
