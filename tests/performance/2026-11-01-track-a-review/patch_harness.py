# Makes ~/sd-review/hharness (a copy of the official harness/harness dir) usable on a docker host with the systemd cgroup driver:
# memory.peak is read from inside the container's own cgroup (as tools/bench.sh does) instead of the parent cgroup /sys/fs/cgroup/speeddemon/run-*.
# Everything else (allowlist check, build, prewarm, 5 runs, sd-timer, sha256 gate, docker diff gate, mid-3 mean, spread rule) is untouched.
import re,os
p=os.path.expanduser('~/sd-review/hharness/run_submission.py'); s=open(p).read()
a='        f"--cgroup-parent=/{CGROUP_PARENT}/{run_id}",\n'; assert a in s; s=s.replace(a,'')
a='''        "--entrypoint", f"/sd-bin/{os.path.basename(timer_bin)}",
        tag, "/sd/timing.json", NOBODY, NOBODY, "--", *cmd,
    ]'''
b='''        "--entrypoint", "/bin/sh",
        tag, "-c", "/sd-bin/%s /sd/timing.json %s %s -- %s; rc=$?; cat /sys/fs/cgroup/memory.peak > /sd/peak.txt; exit $rc" % (os.path.basename(timer_bin), NOBODY, NOBODY, " ".join(cmd)),
    ]'''
assert a in s; s=s.replace(a,b)
a='    peak = cgroups.peak_and_remove(run_id)\n'; assert a in s
s=s.replace(a,'    peak = int(open(os.path.join(sd_dir, "peak.txt")).read().split()[0]) if os.path.exists(os.path.join(sd_dir, "peak.txt")) else 0\n')
open(p,'w').write(s)
