import json, idautils, ida_hexrays, ida_funcs
def ct(out):
    R = {}
    class V(ida_hexrays.ctree_visitor_t):
        def __init__(s):
            ida_hexrays.ctree_visitor_t.__init__(s, ida_hexrays.CV_FAST); s.calls=[]; s.cmp=[]; s.asg=[]; s.ret=[]
        def visit_insn(s, i):
            if i.op == ida_hexrays.cit_switch:
                for c in i.cswitch.cases:
                    for v in c.values: s.cmp.append(int(v))
            elif i.op == ida_hexrays.cit_return and i.creturn.expr.op == ida_hexrays.cot_num:
                s.ret.append(i.creturn.expr.numval())
            return 0
        def visit_expr(s, e):
            o = e.op
            if o == ida_hexrays.cot_call:
                x = e.x
                while x.op == ida_hexrays.cot_cast: x = x.x
                t = hex(x.obj_ea) if x.op == ida_hexrays.cot_obj else "ind"
                a = []
                for g in e.a:
                    while g.op == ida_hexrays.cot_cast: g = g.x
                    a.append(g.numval() if g.op == ida_hexrays.cot_num else None)
                s.calls.append([t, a])
            elif o in (ida_hexrays.cot_eq, ida_hexrays.cot_ne, ida_hexrays.cot_sgt, ida_hexrays.cot_sge, ida_hexrays.cot_slt, ida_hexrays.cot_sle, ida_hexrays.cot_ugt, ida_hexrays.cot_uge, ida_hexrays.cot_ult, ida_hexrays.cot_ule):
                for g in (e.x, e.y):
                    if g.op == ida_hexrays.cot_num: s.cmp.append(g.numval())
            elif o == ida_hexrays.cot_asg and e.y.op == ida_hexrays.cot_num:
                s.asg.append(e.y.numval())
            return 0
    bad = 0
    for f in idautils.Functions():
        try:
            c = ida_hexrays.decompile(f); v = V(); v.apply_to(c.body, None)
            R[hex(f)] = {"k": v.calls, "m": v.cmp, "a": v.asg, "r": v.ret}
        except Exception:
            bad += 1
    json.dump(R, open(out, "w"))
    return len(R), bad
