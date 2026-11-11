import json,re,sys,collections
S=sys.argv[1]
A=json.load(open(S+'/fp348.json')); B=json.load(open(S+'/fp307.json')); M=json.load(open(S+'/map.json'))
R={b:a for a,b in M.items()}
nm=lambda a: re.sub(r'\(.*','',A[a]['n'])
same=diff=0; out=[]
for a,b in M.items():
    n=nm(a)
    if n.startswith(('sub_','`','std::')): continue
    ca=[nm(c) if c in M else nm(c)+'!' for c in A[a]['c']]
    cb=[nm(R[c]) if c in R else '?' for c in B[b]['c']]
    sa=A[a]['s']; sb=B[b]['s']
    if ca==cb and sa==sb: same+=1; continue
    diff+=1
    da=collections.Counter(ca)-collections.Counter(cb); db=collections.Counter(cb)-collections.Counter(ca)
    ta=collections.Counter(sa)-collections.Counter(sb); tb=collections.Counter(sb)-collections.Counter(sa)
    out.append((n,b,dict(da),dict(db),list(ta),list(tb)))
print('same',same,'diff',diff)
json.dump(out,open(S+'/d1.json','w'))
order=sum(1 for o in out if not(o[2] or o[3] or o[4] or o[5]))
print('order-only',order)
for o in sorted(out):
    if o[2] or o[3] or o[4] or o[5]:
        print(o[0],o[1],'| +348:',' '.join('%s%s'%(k,'' if v==1 else 'x%d'%v) for k,v in o[2].items())[:230],'| +307:',' '.join('%sx%d'%(k,v) for k,v in o[3].items())[:120],'| s348',[x[:50] for x in o[4]][:4],'| s307',[x[:50] for x in o[5]][:4])
