"""IDA script. Run it in the LobbyDaemon 348 database (PDB names) with execute_python.

It writes one slim pseudocode file per class prefix to OUT. The output is a reading aid.
Keep the output out of the repository (clean-room rule). Write facts into the specs.

Use: exec(open('tools/ida_job_extract.py').read()); dump_class('CreateCharacterJob')
"""
import os
import re

import ida_hexrays
import ida_name
import idautils
import idc

OUT = os.environ.get("LD_OUT", "/tmp/ld348")
ASG = re.compile(r"^\s*(?:\w+\()?(\w+)\)? = .*;$")


def dn(ea):
    n = ida_name.get_name(ea) or ""
    return idc.demangle_name(n, idc.get_inf_attr(idc.INF_SHORT_DN)) or n


def joinlines(lines):
    out, buf = [], None
    for l in lines:
        buf = l.rstrip() if buf is None else buf + " " + l.strip()
        if buf.count("(") - buf.count(")") <= 0:
            out.append(buf)
            buf = None
    if buf:
        out.append(buf)
    return out


def dce(src):
    """Remove declarations, log calls, and variables that only fed the log calls."""
    L = src.split("\n")
    k = L.index("{") if "{" in L else 0
    head, decl, body = (L[k - 1] if k else ""), [], []
    for l in L[k + 1:]:
        m = re.match(r"^\s+[\w\s\*:<>,]*?[\s\*](\w+)(\[\d+\])*; // ", l)
        if m and not body:
            decl.append(m.group(1))
        elif l.strip():
            body.append(l)
    b2 = []
    for l in joinlines(body):
        m = re.match(r"^(\s*)(Log::Write|AlertSender::Write)\(", l)
        if not m:
            b2.append(l)
        elif m.group(2) == "Log::Write":
            q = re.findall(r'L"((?:[^"\\]|\\.)*)"', l)
            ln = re.search(r'cpp",\s*(\d+)', l)
            b2.append("%s//LOG %s: %s" % (m.group(1), ln.group(1) if ln else "", "".join(q[1:]).split(", tid")[0][:80]))
        else:
            b2.append(m.group(1) + ";")
    body = b2
    for _ in range(12):
        ch = False
        for v in decl:
            occ = [i for i, l in enumerate(body) if re.search(r"\b%s\b" % v, l) and not l.strip().startswith("//")]
            if occ and all(
                ASG.match(body[i]) and ASG.match(body[i]).group(1) == v
                and len(re.findall(r"\b%s\b" % v, body[i])) == 1 and "::Send" not in body[i]
                for i in occ
            ):
                for i in occ:
                    body[i] = re.match(r"^\s*", body[i]).group(0) + ";"
                ch = True
        t = "\n".join(body)
        while True:
            n = re.sub(r"\n\s*\{\n(\s*;\n)+\s*\}", lambda m: "\n" + re.match(r"\n(\s*)", m.group(0)).group(1) + ";", t)
            n = re.sub(r"\n\s*if \(.*\)\n\s*;\n\s*else\n\s*;(?=\n)", "\n;", n)
            n = re.sub(r"\n\s*else\n\s*;(?=\n)", "", n)
            n = re.sub(r"\n\s*if \(.*\)\n\s*;(?=\n(?!\s*else))", "\n;", n)
            n = re.sub(r"(?<=[;{}])\n\s*;(?=\n)", "", n)
            if n == t:
                break
            t, ch = n, True
        body = t.split("\n")
        if not ch:
            break
    return head + "\n" + "\n".join(body)


def squeeze(t):
    o = []
    for l in t.split("\n"):
        s = l.strip()
        if s in ("{", "}", ";"):
            continue
        if re.match(r"^\w+ = (this->GetDefinition\(this\)(->type)?|\w+->type|this->_id|-1);$", s):
            continue
        if "RefCntObject::" in s and "ReferenceCount" in s and not s.startswith("if"):
            s = "//ref"
        o.append(" " * ((len(l) - len(l.lstrip())) // 2) + s)
    return "\n".join(o)


def dump(pred, path):
    parts = []
    for ea in idautils.Functions():
        n = dn(ea)
        if pred(n):
            try:
                s = squeeze(dce(str(ida_hexrays.decompile(ea))))
            except Exception as e:  # noqa: BLE001
                s = "// FAIL %s %s" % (n, e)
            parts.append((ea, "// %x\n%s" % (ea, s)))
    parts.sort()
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w").write("\n\n".join(p for _, p in parts))


def dump_class(cls):
    skip = ("deallocate_me", "_vtbl", "GetDefinition", "destructor")
    dump(lambda n: n.startswith(cls + "::") and not any(x in n for x in skip), "%s/jobs/%s.c" % (OUT, cls))
