#!/usr/bin/env python3
"""Freeze and package baseline, DFS/BFS and separate phase builds."""
from pathlib import Path
import hashlib
import json
import tarfile
import tempfile

artifact = Path(__file__).resolve().parent


def profile(source, reordered):
    def replace(old, new):
        nonlocal source
        assert source.count(old) == 1, old
        source = source.replace(old, new)
    graph = '    let graph = read_graph(&data.join("graph.txt"), workers)?;'
    replace(graph, '    let phase = std::time::Instant::now();\n' + graph + '\n    let graph_s = phase.elapsed().as_secs_f64();\n    let phase = std::time::Instant::now();')
    if reordered:
        reorder = ('    let graph = reorder_graph(graph, &mut queries, workers);'
                   if '    let graph = reorder_graph(graph, &mut queries, workers);' in source
                   else '    let graph = reorder_graph(graph, &mut queries);')
        replace(reorder, '    let query_parse_s = phase.elapsed().as_secs_f64();\n    let phase = std::time::Instant::now();\n' + reorder + '\n    let reorder_s = phase.elapsed().as_secs_f64();')
    else:
        replace('    fs::create_dir_all(&out)?;', '    let query_parse_s = phase.elapsed().as_secs_f64();\n    let reorder_s = 0.0;\n    fs::create_dir_all(&out)?;')
    replace('    for answer in answer_queries(&graph, &queries, workers) {', '    let phase = std::time::Instant::now();\n    let answers = answer_queries(&graph, &queries, workers);\n    let search_s = phase.elapsed().as_secs_f64();\n    let phase = std::time::Instant::now();\n    for answer in answers {')
    fields = ['graph_s', 'query_parse_s', 'reorder_s', 'search_s', 'output_s']
    fmt = 'PHASES={{' + ','.join('"' + name + '":{}' for name in fields) + '}}'
    emit = 'eprintln!(' + json.dumps(fmt) + ', ' + ', '.join(fields) + ');'
    replace('    output.flush()?;', '    output.flush()?;\n    let output_s = phase.elapsed().as_secs_f64();\n    ' + emit)
    return source


def main():
    files = {}
    for variant in ['before', 'dfs', 'bfs']:
        source = (artifact / f'{variant}-main.rs').read_text()
        files[f'{variant}/src/main.rs'] = source.encode()
        profiled = profile(source, variant != 'before')
        (artifact / f'profile-{variant}-main.rs').write_text(profiled)
        files[f'profile-{variant}/src/main.rs'] = profiled.encode()
        for name in ['Cargo.toml', 'Cargo.lock']:
            data = (artifact / ('measured-' + name)).read_bytes()
            files[f'{variant}/{name}'] = data
            files[f'profile-{variant}/{name}'] = data
    template = (artifact / 'measured-parser-tests.rs').read_text()
    for variant in ['dfs', 'bfs']:
        files[f'tests/{variant}-tests.rs'] = template.replace('../track-d/src/main.rs', f'../{variant}/src/main.rs').encode()
    files['tests/traverse-check.py'] = (artifact / 'measured-python-check.py').read_bytes()
    files['tests/reference/d_traverse.py'] = (artifact / 'measured-reference.py').read_bytes()
    for name in ['experiment.py', 'measure.rs']:
        files[name] = (artifact / name).read_bytes()
    metadata = {'baseline_commit': 'e5bad19ffc9c7c18bb543758bde5a33a470bdd8c',
                'variants': 'unchanged mmap/parallel CSR; additional DFS or BFS permutation/CSR rebuild/query remap',
                'sha256': {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}}
    files['source-metadata.json'] = (json.dumps(metadata, indent=2) + '\n').encode()
    (artifact / 'source-metadata.json').write_bytes(files['source-metadata.json'])
    root = Path(tempfile.mkdtemp(prefix='traverse-reorder-payload-'))
    for name, data in files.items():
        target = root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    with tarfile.open('/tmp/traverse-reorder-vps.tar', 'w') as archive:
        for name in files:
            archive.add(root / name, arcname=name)
    print(root)


if __name__ == '__main__':
    main()
