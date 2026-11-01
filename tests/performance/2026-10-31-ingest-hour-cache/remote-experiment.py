"""Read source bundle on stdin; run isolated native Linux A/B and reference checks."""
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import sys
import tempfile
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


bundle = json.load(sys.stdin)
work = Path(tempfile.mkdtemp(prefix="ingest-hour-ab-"))
print("REMOTE_DIR", work, flush=True)
for name, contents in bundle['files'].items():
    path = work / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(contents)
image = 'rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb'
build = ['docker', 'run', '--rm', '--network=none', '-v', f'{work}:/work', '-w', '/work', image]
for variant in ['before', 'track-a']:
    subprocess.run(build + ['cargo', 'build', '--manifest-path', f'{variant}/Cargo.toml', '--release', '--offline', '--locked'], check=True)
subprocess.run(build + ['rustc', '--edition=2024', '--test', '-O', 'tests/ingest-parser.rs', '-o', 'parser-tests'], check=True)
regression = subprocess.check_output([str(work / 'parser-tests')], text=True)
print(regression, flush=True)
(work / 'regression-tests.txt').write_text(regression)
compiler = subprocess.check_output(build + ['rustc', '-Vv'], text=True)
(work / 'rustc-version.txt').write_text(compiler)
(work / 'cpu.txt').write_text(subprocess.check_output(['lscpu'], text=True))
os.sched_setaffinity(0, {2})
result = {'commit': bundle['commit'], 'platform': platform.platform(), 'build_image': image,
          'rustc': compiler, 'cpu_affinity': sorted(os.sched_getaffinity(0)),
          'conditions': 'Native Linux, warm inputs, process spawn to exit; comparisons excluded; no extra RUSTFLAGS; shared host; RSS is not cgroup peak',
          'load_before': os.getloadavg(), 'sources': {n: hashlib.sha256(s.encode()).hexdigest() for n,s in bundle['files'].items()},
          'correctness': [], 'runs': []}
bins = {'before': work/'before/target/release/ingest-baseline', 'after':work/'track-a/target/release/ingest-baseline'}
result['binaries'] = {v:sha(p) for v,p in bins.items()}
data = Path('/home/student/speed-demon-2026/data/rust-validation/a')
cache = Path('/tmp/ingest-byte-linux-mjj_tld6/batch1')
cached = json.loads(Path('/tmp/ingest-byte-linux-mjj_tld6/summary1.json').read_text())
assert cached['source_sha256']['reference'] == sha(work/'tests/reference/a_ingest.py')
prior = {c['case']:c for c in cached['correctness']}
expected = {}
datasets = {}
for name in ['empty', 'edge-cases'] + [f'{s}-seed{seed}' for s in ['tiny','sample'] for seed in [1,7,42]] + ['full-seed1']:
    datasets[name] = data/name
    path=cache/name/'reference/summary.txt'
    assert sha(data/name/'requests.log') == prior[name]['input_sha256']
    assert sha(path) == prior[name]['output_sha256']
    expected[name]=path
for mode, seed in [('alternating',3),('shuffled',19),('same-hour',101)]:
    rng = random.Random(seed)
    name=f'{mode}-seed{seed}'
    folder=work/'synthetic'/name; folder.mkdir(parents=True)
    with (folder/'requests.log').open('w') as out:
        for i in range(300000):
            hour = i%24 if mode == 'alternating' else rng.randrange(24) if mode == 'shuffled' else 17
            latency = [0,4294967295,rng.getrandbits(32)][i%3]
            endpoint=f'/random/{rng.randrange(400)}'
            out.write(f'2026-10-31T{hour:02}:00:00.000000Z GET {endpoint} {rng.choice([200,499,500,599])} {latency}\n')
    subprocess.run([sys.executable,str(work/'tests/reference/a_ingest.py'),'--data',str(folder),'--out',str(folder/'reference')],check=True)
    datasets[name]=folder;expected[name]=folder/'reference/summary.txt'

def execute(variant,name,warmup,round_number,timed):
    output=work/'out'/name/variant;output.mkdir(parents=True,exist_ok=True)
    start=time.perf_counter()
    child=subprocess.Popen([str(bins[variant]),'--data',str(datasets[name]),'--out',str(output)])
    _,status,usage=os.wait4(child.pid,0)
    elapsed=time.perf_counter()-start
    child.returncode=os.waitstatus_to_exitcode(status)
    assert child.returncode==0,(name,variant,child.returncode)
    actual=output/'summary.txt'
    assert actual.read_bytes()==expected[name].read_bytes(),(name,variant)
    assert sha(actual)==sha(expected[name])
    if timed:
        row={'case':name,'variant':variant,'round':round_number,'warmup':warmup,'seconds':elapsed,'rss_bytes':usage.ru_maxrss*1024}
        result['runs'].append(row);print(json.dumps(row),flush=True)

for name in datasets:
    for v in bins: execute(v,name,False,-1,False)
    result['correctness'].append({'case':name,'input_sha256':sha(datasets[name]/'requests.log'),'output_sha256':sha(expected[name]),'bytes_equal':True})
    print('PASS',name,sha(expected[name]),flush=True)
assert sha(expected['full-seed1'])=='495ca09682b9673ef836a506496177680bb4a3af54255174befddb1f83481cd3'
assert sha(expected['sample-seed1'])=='ca6311bbeb536fa55d85265bf6fd3d24cc9586f13de1ac73bd2a23ac3893aac3'
result['timing']={}
for name,rounds in [('full-seed1',11),('sample-seed7',7),('sample-seed42',7),('alternating-seed3',7),('shuffled-seed19',7),('same-hour-seed101',7)]:
    for r in range(rounds):
        for v in (['before','after'] if r%2==0 else ['after','before']):
            execute(v,name,r==0,r,True)
    stats={}
    for v in bins:
        rows=[r['seconds'] for r in result['runs'] if r['case']==name and r['variant']==v and not r['warmup']]
        stats[v]={'median':statistics.median(rows),'trimmed_mean':statistics.mean(sorted(rows)[1:-1]),'spread':max(rows)/min(rows)-1}
    stats['time_ratio']=stats['after']['trimmed_mean']/stats['before']['trimmed_mean']
    result['timing'][name]=stats
    print('SUMMARY',name,json.dumps(stats),flush=True)
result['load_after']=os.getloadavg()
(work/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('__ARTIFACTS__'+json.dumps({n:(work/n).read_text() for n in ['summary.json','cpu.txt','rustc-version.txt','regression-tests.txt']}),flush=True)
