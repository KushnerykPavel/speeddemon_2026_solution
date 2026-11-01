#!/bin/bash
# Board-like run of Track A on the VPS: docker with the harness flags, sd-timer wall time,
# cgroup memory.peak, output sha256. Not the board's own runner (that needs a dedicated docker/cgroup setup).
# usage: tools/bench.sh [label] [cpuset] [runs] [data-case]
set -euo pipefail
cd "$(dirname "$0")/.."
L=${1:-cur}; CPUSET=${2:-0-5}; RUNS=${3:-7}; CASE=${4:-full-seed1}
ssh my-vps "mkdir -p ~/sd-bench/$L/ctx ~/sd-bench/work ~/sd-bench/timer"
rsync -a --delete --exclude target ${SRC:-track-a}/ my-vps:sd-bench/$L/ctx/
ssh my-vps "bash -s" <<REMOTE
set -e
cd ~/sd-bench
[ -x timer/sd-timer ] || gcc -O2 -o timer/sd-timer ~/speed-demon-2026/harness/harness/timer/sd-timer.c
docker build -q --network=none -t sdbench/$L $L/ctx >/dev/null
D=~/speed-demon-2026/data/rust-validation/a/$CASE
cat \$D/requests.log > /dev/null
python3 - <<PY
import subprocess,os,json,hashlib,statistics,glob
L='$L'; D=os.path.expanduser('~/speed-demon-2026/data/rust-validation/a/$CASE'); W=os.path.expanduser('~/sd-bench/work')
res=[]
for i in range($RUNS):
    out=W+'/out'; sd=W+'/sd'
    subprocess.run(['sudo','rm','-rf',out,sd]); os.makedirs(out,mode=0o777); os.makedirs(sd,mode=0o700); os.chmod(out,0o777); subprocess.run(['sudo','chown','root:root',sd])
    cmd=['sudo','docker','run','--rm','--no-healthcheck','--user=0:0','--cpuset-cpus=$CPUSET','--cpuset-mems=0','--memory=8g','--memory-swap=8g',
     '--pids-limit=512','--network=none','--read-only','--tmpfs','/tmp:rw,size=1g,exec','--cap-drop=ALL','--cap-add=SETUID','--cap-add=SETGID',
     '--security-opt=no-new-privileges','-v',D+':/data:ro','-v',out+':/out:rw','-v',sd+':/sd','-v',os.path.expanduser('~/sd-bench/timer')+':/sd-bin:ro',
     '-e','TRACK=a','--entrypoint','/bin/sh','sdbench/'+L,'-c',
     '/sd-bin/sd-timer /sd/timing.json 65534 65534 -- /app/solution; cat /sys/fs/cgroup/memory.peak > /out/peak.txt; grep -E "^(anon|file|shmem|file_mapped|kernel|sock|pagetables) " /sys/fs/cgroup/memory.stat >> /out/peak.txt']
    subprocess.run(cmd,check=True,stdout=subprocess.DEVNULL)
    t=json.loads(subprocess.run(['sudo','cat',sd+'/timing.json'],capture_output=True,text=True).stdout)
    pk=subprocess.run(['sudo','cat',out+'/peak.txt'],capture_output=True,text=True).stdout.split('\\n'); peak=int(pk[0]); stat=' '.join(x.replace(' ','=') for x in pk[1:] if x)
    h=hashlib.sha256(open(out+'/summary.txt','rb').read()).hexdigest() if os.path.exists(out+'/summary.txt') else None
    res.append((t['wall_ns']/1e9,peak,h)); last=stat
walls=sorted(r[0] for r in res); mid=walls[1:-1] if len(walls)>=3 else walls
print('label',L,'case','$CASE','cpuset','$CPUSET','walls',[round(w,4) for w in walls])
print('memory.stat at exit:',last)
print('T(trimmed mean) %.4f s  min %.4f  M(max) %.2f MB  hashes %s'%(statistics.mean(mid),walls[0],max(r[1] for r in res)/1e6,set(r[2][:12] if r[2] else None for r in res)))
PY
REMOTE
