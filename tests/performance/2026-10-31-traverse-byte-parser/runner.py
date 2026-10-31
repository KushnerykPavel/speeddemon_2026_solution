#!/usr/bin/env python3
"""Local synthetic loading benchmark; queries immediately return at their source."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
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
    rows = []
    nodes, edges, queries = 100_000, 1_000_000, 10_000
    with tempfile.TemporaryDirectory(prefix="traverse-byte-bench-") as temporary:
        root = Path(temporary)
        with (root / "graph.txt").open("w") as graph:
            graph.write(f"{nodes} {edges}\n")
            for i in range(edges):
                graph.write(f"{i % nodes} {(i * 7919 + 1) % nodes} {(i * 104729) % 4294967296}\n")
        (root / "queries.txt").write_text("".join(f"{i % nodes} {i % nodes}\n" for i in range(queries)))
        expected = b"0\n" * queries
        for round_number in range(6):
            for variant in (["before", "after"] if round_number % 2 == 0 else ["after", "before"]):
                start = time.perf_counter()
                subprocess.run([str(binaries[variant]), "--data", str(root), "--out", str(root / variant)], check=True)
                seconds = time.perf_counter() - start
                assert (root / variant / "distances.txt").read_bytes() == expected
                rows.append({"round": round_number, "variant": variant, "seconds": seconds, "byte_equal": True})
        graph_hash = hashlib.sha256((root / "graph.txt").read_bytes()).hexdigest()
    medians = {variant: statistics.median(row["seconds"] for row in rows if row["round"] > 0 and row["variant"] == variant)
               for variant in binaries}
    print(json.dumps({
        "platform": platform.platform(), "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "nodes": nodes, "edges": edges, "queries": queries, "graph_sha256": graph_hash,
        "method": "warm cache; one warmup and five alternating runs per variant; median wall time",
        "rows": rows, "median_seconds": medians,
        "time_reduction_percent": 100 * (1 - medians["after"] / medians["before"]),
    }, indent=2))


if __name__ == "__main__":
    main()
