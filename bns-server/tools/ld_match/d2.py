import json,re,sys,collections
S=sys.argv[1]
A=json.load(open(S+'/fp348.json')); B=json.load(open(S+'/fp307.json')); M=json.load(open(S+'/map.json'))
XA=json.load(open(S+'/ct348.json')); XB=json.load(open(S+'/ct307.json'))
R={b:a for a,b in M.items()}
nm=lambda a: re.sub(r'\(.*','',A[a]['n']) if a in A else a
C=collections.Counter
LOG=('Log::Write','AlertSender::Write')
def calls(X,f,side):
    o=[]
    for t,a in X[f]['k']:
        if t!='ind':
            if side: t=R.get(t)
            n=(nm(t) if (side or t in M) else '?') if t else '?'
        else: n='ind'
        if n in LOG or n=='ind' or n=='?' or n.startswith(('0x','sub_','mem','_','LobbyDB::statment','swprintf','j_')) or '!' in n or n.split('::')[-1] in ('removeReferenceCount','addReferenceCount','Checkout','MakeCrash','Load','LogFile'): continue
        o.append((n,tuple(x for x in a if x is not None and x>1 and x<0xffffffff)))
    return C(o)
same=0; rows=[]
for a,b in M.items():
    n=nm(a)
    if n.startswith(('sub_','`','std::')) or a not in XA or b not in XB: continue
    ka=calls(XA,a,0); kb=calls(XB,b,1)
    d={}
    x=ka-kb; y=kb-ka
    # only keep call diffs that carry constants
    x={k:v for k,v in x.items() if k[1]}; y={k:v for k,v in y.items() if k[1]}
    if x or y: d['k']=(x,y)
    for key in 'mr':
        F=lambda L:C(v for v in L if 1<v<0xffffffff); p=F(XA[a][key])-F(XB[b][key]); q=F(XB[b][key])-F(XA[a][key])
        if p or q: d[key]=(dict(p),dict(q))
    if d: rows.append((n,b,d))
    else: same+=1
print('same',same,'diff',len(rows))
for n,b,d in sorted(rows):
    s=n+' '+b
    if 'k' in d:
        f=lambda z:' '.join('%s%s'%(k[0].split('::')[-1],list(k[1])) for k in z)
        s+=' | K348 '+f(d['k'][0])[:200]+' K307 '+f(d['k'][1])[:200]
    for key in 'mr':
        if key in d: s+=' | %s348 %s %s307 %s'%(key,str(d[key][0])[:90],key,str(d[key][1])[:90])
    print(s)
