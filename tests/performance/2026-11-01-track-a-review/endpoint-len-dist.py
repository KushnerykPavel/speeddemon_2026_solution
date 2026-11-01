# Distribution of the endpoint hash loop trip count (len//8 full words, as in State::scan) over the lines of a dataset, plus a
# crude predictability proxy: fraction of lines whose trip count equals the previous line's, and entropy in bits.
import sys,collections,math
c=collections.Counter(); prev=None; same=0; n=0; lens=collections.Counter()
for l in open(sys.argv[1],'rb'):
    p=l.split(b' '); L=len(p[2]); t=L//8; c[t]+=1; lens[L]+=1; n+=1
    same+= (t==prev); prev=t
H=-sum(v/n*math.log2(v/n) for v in c.values())
print('lines',n,'mean endpoint len %.1f'%(sum(k*v for k,v in lens.items())/n))
print('trip count (len//8) distribution:',{k:round(v/n,4) for k,v in sorted(c.items())})
print('entropy of trip count %.2f bits; P(same as previous line) %.3f; P(modal) %.3f'%(H,same/n,max(c.values())/n))
