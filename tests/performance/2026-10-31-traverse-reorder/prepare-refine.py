from pathlib import Path
import hashlib,json,runpy,tarfile,tempfile
artifact=Path(__file__).resolve().parent
profile=runpy.run_path(str(artifact/'prepare.py'))['profile']
files={}
source=(artifact/'bfs-par-main.rs').read_bytes()
files['bfs-par/src/main.rs']=source
profiled=profile(source.decode(),True).encode()
(artifact/'profile-bfs-par-main.rs').write_bytes(profiled)
files['profile-bfs-par/src/main.rs']=profiled
for name in ['Cargo.toml','Cargo.lock']:
 for variant in ['bfs-par','profile-bfs-par']:files[f'{variant}/{name}']=(artifact/('measured-'+name)).read_bytes()
files['tests/bfs-par-tests.rs']=(artifact/'measured-par-parser-tests.rs').read_text().replace('../track-d/src/main.rs','../bfs-par/src/main.rs').encode()
files['refine.py']=(artifact/'refine.py').read_bytes()
metadata={'baseline_commit':'e5bad19ffc9c7c18bb543758bde5a33a470bdd8c','candidate':'BFS mapping, parallel safe row-slice CSR reconstruction',
          'sha256':{name:hashlib.sha256(data).hexdigest() for name,data in files.items()}}
files['refine-source-metadata.json']=(json.dumps(metadata,indent=2)+'\n').encode()
(artifact/'refine-source-metadata.json').write_bytes(files['refine-source-metadata.json'])
root=Path(tempfile.mkdtemp(prefix='traverse-reorder-refine-'))
for name,data in files.items():
 path=root/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(data)
with tarfile.open('/tmp/traverse-reorder-refine.tar','w') as archive:
 for name in files:archive.add(root/name,arcname=name)
print(root)
