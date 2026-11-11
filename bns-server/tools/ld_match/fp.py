import json, idautils, idc, ida_funcs, ida_name, ida_nalt
def fp(out):
    smap = {}
    for s in idautils.Strings(default_setup=False) if False else []: pass
    st = idautils.Strings(); st.setup(strtypes=[ida_nalt.STRTYPE_C, ida_nalt.STRTYPE_C_16], minlen=2)
    for s in st: smap[s.ea] = str(s)
    R = {}
    for f in idautils.Functions():
        fn = ida_funcs.get_func(f); strs = []; calls = []
        for h in idautils.FuncItems(f):
            for d in idautils.DataRefsFrom(h):
                if d in smap: strs.append(smap[d])
                elif ida_funcs.get_func(d) and ida_funcs.get_func(d).start_ea == d: calls.append(d)
            for c in idautils.CodeRefsFrom(h, 0):
                g = ida_funcs.get_func(c)
                if g and g.start_ea == c and c != f: calls.append(c)
        n = ida_name.get_name(f) or ""
        R[hex(f)] = {"n": idc.demangle_name(n, idc.get_inf_attr(idc.INF_SHORT_DN)) or n, "sz": fn.size(), "s": strs, "c": [hex(x) for x in calls]}
    json.dump(R, open(out, "w"))
    return len(R)
