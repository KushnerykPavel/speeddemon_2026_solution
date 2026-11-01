import json, pathlib, subprocess
root=pathlib.Path.cwd()
src=(root/'tests/performance/2026-10-31-ingest-hour-cache/before-main.rs').read_text()
profile=src.replace('let groups = read_groups(&mut reader)?;', '''let started = std::time::Instant::now();
    let groups = read_groups(&mut reader)?;
    let read_s = started.elapsed().as_secs_f64();
    let finalize = std::time::Instant::now();''').replace('    output.flush()?;', '''    output.flush()?;
    eprintln!("PHASE {} {}", read_s, finalize.elapsed().as_secs_f64());''')
files={'before/src/main.rs':src,'profile/src/main.rs':profile}
for name in ['before','profile']:
 for f in ['Cargo.toml','Cargo.lock']:
  files[f'{name}/{f}']=(root/'track-a'/f).read_text()
driver='''import json,sys,tempfile,pathlib,subprocess,os
b=json.load(sys.stdin); w=pathlib.Path(tempfile.mkdtemp(prefix="ingest-hour-")); print("REMOTE_DIR",w,flush=True)
for n,s in b.items():
 p=w/n;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(s)
image="rust:1-slim-trixie@sha256:1a939748a7cbab172de91a014e3804a97af1394a8c844faa67fce9401c4e40fb"
base=["docker","run","--rm","--network=none","-v",f"{w}:/work","-w","/work",image]
for n in ["before","profile"]:
 subprocess.run(base+["cargo","build","--manifest-path",f"{n}/Cargo.toml","--release","--offline","--locked"],check=True)
data=pathlib.Path("/home/student/speed-demon-2026/data/rust-validation/a")
for scale in ["tiny-seed1","sample-seed1","sample-seed7","sample-seed42","full-seed1"]:
 count=changes=0;last=None
 with (data/scale/"requests.log").open("rb") as f:
  for row in f:
   hour=row[:13];changes+=hour!=last;last=hour;count+=1
 print("LOCALITY",scale,count,changes,flush=True)
for r in range(5):
 subprocess.run(["taskset","-c","2",str(w/"profile/target/release/ingest-baseline"),"--data",str(data/"full-seed1"),"--out",str(w/"profile-out")],check=True)
print("LOAD",os.getloadavg(),flush=True)
'''
cmd=['ssh','my-vps','python3 -c '+__import__('shlex').quote(driver)]
p=subprocess.run(cmd,input=json.dumps(files),text=True,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
(root/'tests/performance/2026-10-31-ingest-hour-cache/diagnostic.txt').write_text(p.stdout)
print(p.stdout);p.check_returncode()
