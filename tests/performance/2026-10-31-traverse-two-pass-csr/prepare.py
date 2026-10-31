#!/usr/bin/env python3
"""Prepare exact source snapshots and separately instrumented profiling builds."""
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile

artifact = Path(__file__).resolve().parent
repo = artifact.parents[2]


def emit(prefix, fields):
    fmt = prefix + '{{' + ','.join('"' + name + '":{}' for name in fields) + '}}'
    return 'eprintln!(' + json.dumps(fmt) + ', ' + ', '.join(fields.values()) + ');'


def profile(source, two_pass):
    def replace(old, new):
        nonlocal source
        assert source.count(old) == 1, old
        source = source.replace(old, new)
    replace('    let (n, m) = (n as usize, m as usize);',
            '    let (n, m) = (n as usize, m as usize);\n    let graph_stage = std::time::Instant::now();')
    replace('    for i in 0..n {',
            '    let graph_first_pass_s = graph_stage.elapsed().as_secs_f64();\n    let graph_stage = std::time::Instant::now();\n    for i in 0..n {')
    anchor = '    reader.rewind()?;' if two_pass else '    for (u, v, w) in list {'
    replace(anchor, '    let graph_prefix_allocate_s = graph_stage.elapsed().as_secs_f64();\n    let graph_stage = std::time::Instant::now();\n' + anchor)
    replace('    Ok(Graph {', '    let graph_scatter_s = graph_stage.elapsed().as_secs_f64();\n    ' + emit('GRAPH_PHASES=', {
        'first_pass_s': 'graph_first_pass_s', 'prefix_allocate_s': 'graph_prefix_allocate_s',
        'scatter_s': 'graph_scatter_s'}) + '\n    Ok(Graph {')
    replace('    let graph = read_graph(&data.join("graph.txt"))?;',
            '    let phase = std::time::Instant::now();\n    let graph = read_graph(&data.join("graph.txt"))?;\n    let graph_s = phase.elapsed().as_secs_f64();\n    let phase = std::time::Instant::now();')
    replace('    fs::create_dir_all(&out)?;',
            '    let query_parse_s = phase.elapsed().as_secs_f64();\n    fs::create_dir_all(&out)?;')
    replace('    for answer in answer_queries(&graph, &queries, workers) {',
            '    let phase = std::time::Instant::now();\n    let answers = answer_queries(&graph, &queries, workers);\n    let search_s = phase.elapsed().as_secs_f64();\n    let phase = std::time::Instant::now();\n    for answer in answers {')
    replace('    output.flush()?;', '    output.flush()?;\n    let output_s = phase.elapsed().as_secs_f64();\n    ' + emit('PHASES=', {
        'graph_s': 'graph_s', 'query_parse_s': 'query_parse_s', 'search_s': 'search_s', 'output_s': 'output_s'}))
    return source


def main():
    root = Path(tempfile.mkdtemp(prefix='traverse-two-pass-payload-'))
    files = {}
    for variant, filename in [('before', 'before-main.rs'), ('track-d', 'two-pass-main.rs')]:
        source = (artifact / filename).read_text()
        files[variant + '/src/main.rs'] = source.encode()
        profiled = profile(source, variant == 'track-d')
        profile_name = 'profile-before' if variant == 'before' else 'profile-after'
        (artifact / (profile_name + '-main.rs')).write_text(profiled)
        files[profile_name + '/src/main.rs'] = profiled.encode()
        for name in ['Cargo.toml', 'Cargo.lock']:
            files[variant + '/' + name] = (repo / 'track-d' / name).read_bytes()
            files[profile_name + '/' + name] = files[variant + '/' + name]
    for name in ['tests/traverse-parser.rs', 'tests/traverse-check.py', 'tests/reference/d_traverse.py']:
        files[name] = ((artifact / 'measured-parser-tests.rs') if name == 'tests/traverse-parser.rs' else (repo / name)).read_bytes()
    for name in ['experiment.py', 'measure.rs']:
        files[name] = (artifact / name).read_bytes()
    metadata = {'baseline_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True).strip(),
                'candidate': 'two-pass CSR, otherwise identical D7 source',
                'sha256': {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}}
    files['source-metadata.json'] = (json.dumps(metadata, indent=2) + '\n').encode()
    (artifact / 'source-metadata.json').write_bytes(files['source-metadata.json'])
    for name, data in files.items():
        target = root / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    with tarfile.open('/tmp/traverse-two-pass-vps.tar', 'w') as archive:
        for name in files:
            archive.add(root / name, arcname=name)
    print(root)


if __name__ == '__main__':
    main()
