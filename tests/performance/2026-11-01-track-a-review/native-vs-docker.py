#!/usr/bin/env python3
"""VPS: same HEAD binary (bin-cur, extracted from sdrev/cur) timed by sd-timer natively as uid 0 (home dir is not traversable by nobody) (taskset 0-5, output on /dev/shm)
vs inside the board-like docker run (output on /dev/shm/sdw). Alternating order, N rounds. JSON lines."""
import subprocess,os,json,sys,time
H=os.path.expanduser('~'); D=H+'/speed-demon-2026/data/rust-validation/a/full-seed1'; W='/dev/shm/sdw'; N=int(sys.argv[1])
def rd(p): return json.loads(subprocess.run(['sudo','cat',p],capture_output=True,text=True).stdout)['wall_ns']/1e9
def native():
    out='/dev/shm/natout'; sd='/dev/shm/natsd'
    subprocess.run(['sudo','rm','-rf',out,sd]); os.makedirs(out,mode=0o777); os.chmod(out,0o777); os.makedirs(sd,mode=0o700)
    subprocess.run(['sudo','taskset','-c','0-5',H+'/sd-review/timer/sd-timer',sd+'/timing.json','0','0','--',H+'/sd-review/bin-cur','--data',D,'--out',out],check=True)
    return rd(sd+'/timing.json')
def docker():
    out=W+'/out'; sd=W+'/sd'
    subprocess.run(['sudo','rm','-rf',out,sd]); os.makedirs(out,mode=0o777); os.makedirs(sd,mode=0o700); os.chmod(out,0o777); subprocess.run(['sudo','chown','root:root',sd])
    cmd=['sudo','docker','run','--rm','--no-healthcheck','--user=0:0','--cpuset-cpus=0-5','--cpuset-mems=0','--memory=8g','--memory-swap=8g','--pids-limit=512','--network=none','--read-only','--tmpfs','/tmp:rw,size=1g,exec','--cap-drop=ALL','--cap-add=SETUID','--cap-add=SETGID','--security-opt=no-new-privileges','-v',D+':/data:ro','-v',out+':/out:rw','-v',sd+':/sd','-v',H+'/sd-review/timer:/sd-bin:ro','-e','TRACK=a','--entrypoint','/sd-bin/sd-timer','sdrev/cur','/sd/timing.json','65534','65534','--','/app/solution']
    subprocess.run(cmd,check=True,stdout=subprocess.DEVNULL)
    return rd(sd+'/timing.json')
subprocess.run(['cat',D+'/requests.log'],stdout=subprocess.DEVNULL); native(); docker()
for r in range(N):
    for k in (('native','docker') if r%2==0 else ('docker','native')):
        print(json.dumps(dict(round=r,mode=k,wall_s=(native if k=='native' else docker)())),flush=True)
