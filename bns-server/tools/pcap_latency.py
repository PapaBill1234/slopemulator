#!/usr/bin/env python3
"""Server response times from a capture (rule M1 of Docs/Architecture/scale-rules.md).

Usage: tools/pcap_latency.py <cap.pcap> [--slow MS] [--ports PORT:NAME,...]

For each TCP connection to a server port: the time from a client data packet
to the next server data packet. The capture runs on the server host, so this
time is the server-side part. Output: one table row for each connection and
one line for each response slower than --slow (default 20 ms).
"""
import re, subprocess, sys
from collections import defaultdict

SERVER_PORTS = {6600: "CliGate", 10900: "LobbyGate", 10241: "GameDaemon"}
LINE = re.compile(r"^(\d+\.\d+) IP6? (\S+)\.(\d+) > (\S+)\.(\d+): Flags \[([^\]]*)\].* length (\d+)")

def main():
    cap = sys.argv[1]
    slow = float(sys.argv[sys.argv.index("--slow") + 1]) if "--slow" in sys.argv else 20.0
    if "--ports" in sys.argv:
        SERVER_PORTS.clear()
        for item in sys.argv[sys.argv.index("--ports") + 1].split(","):
            port, name = item.split(":")
            SERVER_PORTS[int(port)] = name
    out = subprocess.run(["tcpdump", "-tt", "-n", "-r", cap, "tcp"], capture_output=True, text=True).stdout
    conns = defaultdict(list)  # (server port, client addr, client port) -> [(time, from_client, flags, length)]
    for line in out.splitlines():
        m = LINE.match(line)
        if not m:
            continue
        t, src, sp, dst, dp, flags, length = float(m[1]), m[2], int(m[3]), m[4], int(m[5]), m[6], int(m[7])
        if dp in SERVER_PORTS:
            conns[(dp, src, sp)].append((t, True, flags, length))
        elif sp in SERVER_PORTS:
            conns[(sp, dst, dp)].append((t, False, flags, length))
    if not conns:
        print("no connection to a known server port")
        return
    t0 = min(p[0][0] for p in conns.values())
    print("| Start (s) | Service | Client port | Life (ms) | Requests | Response p50 (ms) | Response max (ms) | Client idle (ms) |")
    print("| --- | --- | --- | --- | --- | --- | --- | --- |")
    slow_rows = []
    for key in sorted(conns, key=lambda k: conns[k][0][0]):
        pk = conns[key]
        gaps, idle, wait, last_srv = [], 0.0, None, None
        for t, from_client, flags, length in pk:
            if length == 0:
                continue
            if from_client:
                if wait is None:
                    wait = t
                    if last_srv is not None:
                        idle += t - last_srv
            elif wait is not None:
                gap = (t - wait) * 1000
                gaps.append(gap)
                if gap >= slow:
                    slow_rows.append((wait - t0, SERVER_PORTS[key[0]], key[2], gap, length))
                wait, last_srv = None, t
            else:
                last_srv = t
        gaps.sort()
        p50 = gaps[len(gaps) // 2] if gaps else 0.0
        mx = gaps[-1] if gaps else 0.0
        print(f"| {pk[0][0]-t0:.3f} | {SERVER_PORTS[key[0]]} | {key[2]} | {(pk[-1][0]-pk[0][0])*1000:.0f} | {len(gaps)} | {p50:.2f} | {mx:.2f} | {idle*1000:.0f} |")
    print(f"\nResponses of {slow:g} ms or more:")
    for t, svc, port, gap, length in slow_rows:
        print(f"  {t:.3f} s  {svc} (client port {port})  {gap:.1f} ms  first reply {length} B")
    if not slow_rows:
        print("  none")

if __name__ == "__main__":
    main()
