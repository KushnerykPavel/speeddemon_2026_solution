import json,sys,statistics as s,random
rows=[json.loads(l) for l in open(sys.argv[1])]
A,B=sys.argv[2],sys.argv[3]
def st(img):
    w=sorted(r['wall_s'] for r in rows if r['img']==img); n=len(w)
    mid=w[1:-1]; q=w[n//4:n-n//4]
    return w,'n=%d min %.4f p25 %.4f med %.4f mean %.4f trimmed(drop 1 ea) %.4f mid-3-of-5-style n/a  p75 %.4f max %.4f sd %.4f  (max-min)/med %.0f%%  peak max %.2f MB'%(n,w[0],w[n//4],s.median(w),s.mean(w),s.mean(mid),w[3*n//4],w[-1],s.stdev(w),100*(w[-1]-w[0])/s.median(w),max(r['peak'] for r in rows if r['img']==img)/1e6)
for i in (A,B): print(i,st(i)[1])
by={}
for r in rows: by.setdefault(r['round'],{})[r['img']]=r['wall_s']
d=[by[k][A]-by[k][B] for k in sorted(by)]
print('paired diff %s - %s per round: median %.4f mean %.4f  (negative = %s faster); A faster in %d/%d rounds'%(A,B,s.median(d),s.mean(d),A,sum(x<0 for x in d),len(d)))
random.seed(1); bs=[s.median(random.choices(d,k=len(d))) for _ in range(5000)]; bs.sort()
print('bootstrap 95%% CI of median paired diff: [%.4f, %.4f]'%(bs[125],bs[4875]))
# first-run-in-round vs second-run effect
print('sha set',set(r['sha'][:12] for r in rows))
