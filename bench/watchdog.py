#!/usr/bin/env python3
"""Thermal watchdog for long benchmark runs on the phone (run inside Termux).

Every minute it logs battery and CPU temperature to ~/bench/results/temp.log and kills the benchmark chain
(chain.sh, bench.py, llama-server) if the battery gets too hot, the battery health is not GOOD, or the CPU is too hot.
Killed runs resume from their *.partial.jsonl files. A marker file watchdog.killed records why.

Usage: python watchdog.py [--warn 42] [--kill 45] [--cpu-kill 85] [--interval 60] [--once] [--dry]
"""
import argparse, json, os, subprocess, time

RES = os.path.expanduser("~/bench/results")
LOG, MARK = f"{RES}/temp.log", f"{RES}/watchdog.killed"


def battery():
    out = subprocess.run(["termux-battery-status"], capture_output=True, text=True, timeout=25).stdout
    d = json.loads(out)
    return float(d["temperature"]), d.get("health", "?"), d.get("percentage"), d.get("status", "?")


def cpu_max():
    best = 0.0
    for i in range(40):
        try:
            if open(f"/sys/class/thermal/thermal_zone{i}/type").read().strip().startswith("tsens_tz_sensor"):
                v = float(open(f"/sys/class/thermal/thermal_zone{i}/temp").read())
                best = max(best, v if v < 200 else v / 1000)   # some kernels report milli-degrees
        except (OSError, ValueError):
            pass
    return best


def log(msg):
    with open(LOG, "a") as f:
        f.write(f"{time.strftime('%Y-%m-%d %H:%M:%S')} {msg}\n")


def kill_all():
    for pat in ("chain.sh", "bench.py", "llama-server"):
        subprocess.run(["pkill", "-f", pat])
    subprocess.run(["termux-wake-unlock"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--warn", type=float, default=42.0)
    ap.add_argument("--kill", type=float, default=45.0)
    ap.add_argument("--cpu-kill", type=float, default=85.0)
    ap.add_argument("--min-charge", type=int, default=10, help="kill if charge is at or below this percent and not charging")
    ap.add_argument("--interval", type=int, default=60)
    ap.add_argument("--once", action="store_true")
    ap.add_argument("--dry", action="store_true", help="log what would be killed, kill nothing")
    a = ap.parse_args()
    if os.path.exists(MARK) and not a.dry:
        os.remove(MARK)
    log(f"watchdog start warn={a.warn} kill={a.kill} cpu_kill={a.cpu_kill} dry={a.dry}")
    while True:
        try:
            bat, health, pct, status = battery()
        except Exception as e:                      # sensor read failed: log and try again, never kill on a read error
            log(f"read error: {e!r}")
            if a.once:
                return
            time.sleep(a.interval)
            continue
        cpu = cpu_max()
        level = "WARN" if bat >= a.warn else "ok"
        log(f"bat={bat:.1f}C cpu={cpu:.0f}C health={health} charge={pct}% {status} {level}")
        reason = None
        if bat >= a.kill:
            reason = f"battery {bat:.1f}C >= {a.kill}C"
        elif pct is not None and pct <= a.min_charge and status == "DISCHARGING":
            reason = f"battery charge {pct}% and discharging"
        elif health != "GOOD":
            reason = f"battery health {health}"
        elif cpu >= a.cpu_kill:
            reason = f"cpu {cpu:.0f}C >= {a.cpu_kill}C"
        if reason:
            log(f"{'WOULD KILL' if a.dry else 'KILL'}: {reason}")
            if not a.dry:
                kill_all()
                open(MARK, "w").write(reason + "\n")
                return
        if a.once:
            return
        time.sleep(a.interval)


if __name__ == "__main__":
    main()
