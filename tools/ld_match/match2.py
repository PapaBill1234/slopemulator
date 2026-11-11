import json, collections, sys, re, os
S=sys.argv[1]
A=json.load(open(S+'/fp348.json')); B=json.load(open(S+'/fp307.json'))
M=json.load(open(S+'/map.json')); R={b:a for a,b in M.items()}
def add(a,b):
    if a in M or b in R or a not in A or b not in B: return False
    M[a]=b; R[b]=a; return True
# callers
def callers(X):
    d=collections.defaultdict(list)
    for f,v in X.items():
        for c in v['c']: d[c].append(f)
    return d
CA=callers(A); CB=callers(B)
T={}
if os.path.exists(S+'/tab348.json'):
    TA=json.load(open(S+'/tab348.json')); TB=json.load(open(S+'/tab307.json'))
else: TA=TB=[]
def rounds():
    n0=len(M)
    while True:
        n=len(M)
        for a,b in list(M.items()):
            ca=A[a]['c']; cb=B[b]['c']
            if len(ca)==len(cb):
                for x,y in zip(ca,cb): add(x,y)
        # signature refinement
        def sig(X,f,mp,side):
            v=X[f]
            cs=tuple((c if side else mp.get(c,'?')) if (c in mp) else '?' for c in v['c'])
            cl=tuple(sorted((c if side else mp[c]) for c in (CA if not side else CB)[f] if c in mp))
            return (tuple(v['s']),cs,cl)
        ka=collections.defaultdict(list); kb=collections.defaultdict(list)
        for a in A:
            if a not in M: ka[sig(A,a,M,0)].append(a)
        for b in B:
            if b not in R: kb[sig(B,b,R,1)].append(b)
        for k,v in ka.items():
            if len(v)==1 and len(kb.get(k,[]))==1 and (k[0] or any(x!='?' for x in k[1]) or k[2]): add(v[0],kb[k][0])
        # tables
        ia={};
        for ti,run in enumerate(TB):
            for j,f in enumerate(run): ia.setdefault(f,[]).append((ti,j))
        for run in TA:
            votes=collections.Counter()
            for j,f in enumerate(run):
                if f in M:
                    for ti,jj in ia.get(M[f],[]): votes[(ti,jj-j)]+=1
            if votes:
                (ti,off),c=votes.most_common(1)[0]
                rb=TB[ti]
                if len(rb)==len(run) and off==0 and c>=1:
                    for x,y in zip(run,rb): add(x,y)
        if len(M)==n: break
rounds()
json.dump(M,open(S+'/map.json','w'))
named=[a for a in A if not A[a]['n'].startswith('sub_')]
un=[A[a]['n'] for a in named if a not in M]
print('total',len(M),'unmatched named',len(un))
cls=collections.Counter(re.split('::',n)[0] for n in un)
print(cls.most_common(45))
