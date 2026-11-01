import json,sys,statistics as s
for f in sys.argv[1:]:
    rows=[json.loads(l) for l in open(f)]
    d=lambda a,b,r:r['stages'][b]-(r['stages'][a] if a else 0)
    seg=[(None,'parse_start','pre'),('parse_start','parse_end','parse'),('parse_end','rank_end','rank'),('rank_end','hours_end','hours'),('hours_end','text_start','mid'),('text_start','text_end','text'),('text_end','before_exit','post')]
    out={n:s.median(d(a,b,r) for r in rows) for a,b,n in seg}
    inproc=s.median(r['stages']['before_exit'] for r in rows); wall=s.median(r['wall_us'] for r in rows)
    print('%-28s n=%d thr=%s wall med %6.1f ms (min %.1f) | '%(f,len(rows),rows[0]['threads'],wall/1e3,min(r['wall_us'] for r in rows)/1e3)+' '.join('%s %.1f'%(k,v/1e3) for k,v in out.items())+' | exec+exit %.1f ms'%(s.median(r['wall_us']-r['stages']['before_exit'] for r in rows)/1e3))
