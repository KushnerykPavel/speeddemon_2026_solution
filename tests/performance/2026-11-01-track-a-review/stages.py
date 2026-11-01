#!/usr/bin/env python3
"""Stage timing of the instrumented copy (sdrev/inst = HEAD source + SD_STAGES marks, see inst.patch.txt) in the
board-like docker run. usage: stages.py CASE CPUSET RUNS THREADS -> JSON lines"""
import subprocess,os,json,sys,re
case,cpuset,runs,threads=sys.argv[1],sys.argv[2],int(sys.argv[3]),sys.argv[4]
H=os.path.expanduser('~'); D=H+'/speed-demon-2026/data/rust-validation/a/'+case; W=os.environ.get('SD_WORK',H+'/sd-review/work')
subprocess.run(['cat',D+'/requests.log'],stdout=subprocess.DEVNULL)
for i in range(runs+1):
    out=W+'/out'; sd=W+'/sd'
    subprocess.run(['sudo','rm','-rf',out,sd]); os.makedirs(out,mode=0o777); os.makedirs(sd,mode=0o700); os.chmod(out,0o777); subprocess.run(['sudo','chown','root:root',sd])
    cmd=['sudo','docker','run','--rm','--no-healthcheck','--user=0:0','--cpuset-cpus='+cpuset,'--cpuset-mems=0','--memory=8g','--memory-swap=8g','--pids-limit=512','--network=none','--read-only','--tmpfs','/tmp:rw,size=1g,exec','--cap-drop=ALL','--cap-add=SETUID','--cap-add=SETGID','--ulimit','nofile=8192:8192','--security-opt=no-new-privileges','-v',D+':/data:ro','-v',out+':/out:rw','-v',sd+':/sd','-v',H+'/sd-review/timer:/sd-bin:ro','-e','TRACK=a','-e','SD_STAGES=1','--entrypoint','/bin/sh',os.environ.get('SD_IMG','sdrev/inst'),'-c','/sd-bin/sd-timer /sd/timing.json 65534 65534 -- /app/solution --threads '+threads]
    p=subprocess.run(cmd,capture_output=True,text=True,check=True)
    if i==0: continue
    t=json.loads(subprocess.run(['sudo','cat',sd+'/timing.json'],capture_output=True,text=True).stdout)
    st={m[0]:int(m[1]) for m in re.findall(r'STAGE (\w+) (\d+)',p.stderr)}
    mem=re.findall(r'MEM (\w+) current=(\d+) peak=(\d+) anon=(\d+) file=(\d+)',p.stderr)
    ru=re.findall(r'RU (\w+) user_us=(\d+) sys_us=(\d+) minflt=(\d+)',p.stderr)
    sums=dict((k,int(v)) for k,v in re.findall(r'(\w+) (\d+)',p.stderr.split('SUMS_US')[1]))
    print(json.dumps(dict(threads=threads,wall_us=t['wall_ns']/1e3,stages=st,sums=sums,mem=mem,ru=ru)),flush=True)
