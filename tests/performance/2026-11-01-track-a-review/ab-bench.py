#!/usr/bin/env python3
"""Interleaved A/B board-like runs on the VPS (same docker flags, sd-timer, cgroup memory.peak as tools/bench.sh).
usage: ab-bench.py CASE CPUSET ROUNDS IMG_A IMG_B [extra solution args]   -> JSON lines on stdout
Order alternates every round (A,B / B,A / ...). Strictly serial."""
import subprocess,os,json,hashlib,sys,time
case,cpuset,rounds,imgs=sys.argv[1],sys.argv[2],int(sys.argv[3]),sys.argv[4:6]
extra=' '.join(sys.argv[6:])
H=os.path.expanduser('~')
D=H+'/speed-demon-2026/data/rust-validation/a/'+case if '/' not in case else case
W=os.environ.get('SD_WORK',H+'/sd-review/work')
def run(img):
    out=W+'/out'; sd=W+'/sd'
    subprocess.run(['sudo','rm','-rf',out,sd]); os.makedirs(out,mode=0o777); os.makedirs(sd,mode=0o700); os.chmod(out,0o777); subprocess.run(['sudo','chown','root:root',sd])
    cmd=['sudo','docker','run','--rm','--no-healthcheck','--user=0:0','--cpuset-cpus='+cpuset,'--cpuset-mems=0','--memory=8g','--memory-swap=8g',
     '--pids-limit=512','--network=none','--read-only','--tmpfs','/tmp:rw,size=1g,exec','--cap-drop=ALL','--cap-add=SETUID','--cap-add=SETGID','--ulimit','nofile=8192:8192',
     '--security-opt=no-new-privileges','-v',D+':/data:ro','-v',out+':/out:rw','-v',sd+':/sd','-v',H+'/sd-review/timer:/sd-bin:ro',
     '-e','TRACK=a','--entrypoint','/bin/sh',img,'-c',
     '/sd-bin/sd-timer /sd/timing.json 65534 65534 -- /app/solution %s; cat /sys/fs/cgroup/memory.peak > /out/peak.txt'%extra]
    subprocess.run(cmd,check=True,stdout=subprocess.DEVNULL)
    t=json.loads(subprocess.run(['sudo','cat',sd+'/timing.json'],capture_output=True,text=True).stdout)
    peak=int(subprocess.run(['sudo','cat',out+'/peak.txt'],capture_output=True,text=True).stdout.split()[0])
    h=hashlib.sha256(open(out+'/summary.txt','rb').read()).hexdigest()
    return t['wall_ns']/1e9,peak,h
# warmup: cat input into page cache + one untimed run of each
subprocess.run(['cat',D+'/requests.log'],stdout=subprocess.DEVNULL)
for i in imgs: run(i)
for r in range(rounds):
    order=imgs if r%2==0 else imgs[::-1]
    for img in order:
        w,p,h=run(img)
        print(json.dumps(dict(round=r,img=img,wall_s=w,peak=p,sha=h,load=open('/proc/loadavg').read().split()[0],t=time.time())),flush=True)
