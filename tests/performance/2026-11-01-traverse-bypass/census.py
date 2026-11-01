import sys, collections
d=sys.argv[1]
f=open(d+'/graph.txt','rb'); n,m=map(int,f.readline().split())
outs=collections.defaultdict(list); ins=collections.defaultdict(list)
for line in f:
    u,v,w=line.split(); u=int(u); v=int(v)
    if u==v: continue
    outs[u].append(v); ins[v].append(u)
q=set()
for line in open(d+'/queries.txt','rb'):
    s,t=map(int,line.split()); q.add(s); q.add(t)
one=two=0; e1=e2=0; deg=collections.Counter()
for v in range(n):
    o=outs.get(v,[]); i=ins.get(v,[])
    deg[(len(i),len(o))]+=1
    if v in q: continue
    if len(o)==1 and len(i)==1 and o[0]!=i[0]: one+=1; e1+=2
    elif len(o)==2 and len(i)==2 and set(o)==set(i) and len(set(o))==2: two+=1; e2+=4
print('n',n,'m',m)
print('one-way chain vertices',one,'edges',e1)
print('two-way degree-2 vertices',two,'edges',e2, 'frac nodes %.1f%% frac edges %.1f%%'%(100*(one+two)/n,100*(e1+e2)/m))
print('top (in,out) degree classes',deg.most_common(12))
