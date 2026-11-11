import json, collections, sys
S=sys.argv[1]
A=json.load(open(S+'/fp348.json')); B=json.load(open(S+'/fp307.json'))
M={}; R={}
def add(a,b):
    if a in M or b in R or a not in A or b not in B: return False
    M[a]=b; R[b]=a; return True
ct=json.load(open('Docs/Server/LobbyDaemon/command-tables.json'))
for t in ct['tables']:
    for r in t['rows']:
        if r.get('guide_handler') and r.get('target_handler'): add(r['guide_handler'],r['target_handler'])
seed=len(M)
# unique string-set match
def key(f): return tuple(sorted(set(f['s'])))
ka=collections.defaultdict(list); kb=collections.defaultdict(list)
for a,f in A.items():
    if f['s']: ka[key(f)].append(a)
for b,f in B.items():
    if f['s']: kb[key(f)].append(b)
for k,v in ka.items():
    if len(v)==1 and len(kb.get(k,[]))==1: add(v[0],kb[k][0])
us=len(M)-seed
# unique single-string match (string used by exactly one function on each side)
sa=collections.defaultdict(set); sb=collections.defaultdict(set)
for a,f in A.items():
    for s in f['s']: sa[s].add(a)
for b,f in B.items():
    for s in f['s']: sb[s].add(b)
for s,v in sa.items():
    if len(v)==1 and len(sb.get(s,()))==1: add(next(iter(v)),next(iter(sb[s])))
# propagate through callee lists
ch=True
while ch:
    ch=False
    for a,b in list(M.items()):
        ca=A[a]['c']; cb=B[b]['c']
        if len(ca)==len(cb):
            for x,y in zip(ca,cb):
                if x in M:
                    continue
                if y in R: continue
                if add(x,y): ch=True
print('seed',seed,'uniq',us,'total',len(M),'of',len(A),len(B))
json.dump(M,open(S+'/map.json','w'))
# report
named=[a for a in A if not A[a]['n'].startswith('sub_')]
un=[A[a]['n'] for a in named if a not in M]
print('named',len(named),'unmatched named',len(un))
import re
cls=collections.Counter(re.split('::',n)[0] for n in un)
print(cls.most_common(40))
