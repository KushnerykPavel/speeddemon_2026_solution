#!/usr/bin/env python3
"""Compare a Track D executable with the preserved Python reference."""
import argparse
import json
from pathlib import Path
import random
import runpy
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve()
    reference = runpy.run_path(str(Path(__file__).parent / "reference/d_traverse.py"))
    cases = [
        ("1 0\n", ""),
        ("3 0\r\n", "0 0\r\n0 2"),
        ("4 5\n0 1 4294967295\n1 2 4294967295\n0 1 4294967295\n2 2 0\n2 0 0", "0 2\n2 1\n0 3\n3 3\n"),
    ]
    for seed in range(200):
        rng = random.Random(seed)
        n = rng.randrange(1, 35)
        edges = [(rng.randrange(n), rng.randrange(n), rng.choice([0, 1, 7, 4294967295, rng.randrange(10000)]))
                 for _ in range(rng.randrange(n * 6))]
        queries = [(rng.randrange(n), rng.randrange(n)) for _ in range(40)]
        newline = "\r\n" if seed % 2 else "\n"
        separator = " \t" if seed % 3 else " "
        graph = newline.join([f"{n} {len(edges)}"] + [separator.join(map(str, edge)) for edge in edges])
        query = newline.join(separator.join(map(str, pair)) for pair in queries)
        if seed % 4:
            graph += newline
            query += newline
        cases.append((graph, query))
    with tempfile.TemporaryDirectory(prefix="traverse-check-") as temporary:
        root = Path(temporary)
        actual, expected = root / "actual", root / "expected"
        expected.mkdir()
        for graph, queries in cases:
            (root / "graph.txt").write_bytes(graph.encode("ascii"))
            (root / "queries.txt").write_bytes(queries.encode("ascii"))
            reference["main"](str(root), str(expected))
            subprocess.run([str(binary), "--data", str(root), "--out", str(actual)], check=True)
            assert (actual / "distances.txt").read_bytes() == (expected / "distances.txt").read_bytes()
        invalid = [
            ("", ""), ("2 1\n0 1 4294967296\n", ""),
            ("2 2\n0 1 3\n", ""), ("2 1\n0 2 3\n", ""),
            ("2 1\n0 1\n", ""), ("2 0\n", "0 2\n"),
            ("2 0\n", "0\n"), ("2 0\n", "0 1 2\n"),
        ]
        for graph, queries in invalid:
            (root / "graph.txt").write_text(graph)
            (root / "queries.txt").write_text(queries)
            result = subprocess.run([str(binary), "--data", str(root), "--out", str(actual)], capture_output=True)
            assert result.returncode != 0
            assert b"panicked" not in result.stderr
    print(json.dumps({"python_byte_equal_cases": len(cases), "invalid_input_cases": len(invalid)}))


if __name__ == "__main__":
    main()
