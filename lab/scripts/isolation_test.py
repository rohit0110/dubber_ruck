import os, socket, urllib.request, subprocess
def t(name, fn):
    try:
        r = fn(); print(f"{'LEAK' if r is True else 'blocked'}: {name}" + (f" -> {r}" if r not in (True, False, None) else ""))
    except Exception as e:
        print(f"blocked: {name} ({type(e).__name__}: {str(e)[:60]})")
def net():
    urllib.request.urlopen("https://huggingface.co", timeout=4); return True
def dns():
    socket.gethostbyname("example.com"); return True
def write(path):
    def f():
        open(path, "w").write("x"); return True
    return f
t("internet access", net); t("DNS lookup", dns)
t("write to root filesystem (/etc/x)", write("/etc/x")); t("write into mounted data/", write("/work/data/x")); t("write into mounted bench/", write("/work/bench/x"))
t("write into mounted tests/", write("/work/tests/x"))
t("see the Mac's /Users folder", lambda: os.path.exists("/Users") or None)
t("see the PDF anywhere", lambda: any("pdf" in f.lower() for r, d, fs in os.walk("/work") for f in fs) or None)
print("user id:", os.getuid(), "| capabilities:", [l.split()[1] for l in open("/proc/self/status") if l.startswith("CapEff")])
t("become root (setuid)", lambda: (os.setuid(0), True)[1])
print("allowed: write to /work/out ->", end=" "); open("/work/out/isolation_ok.txt", "w").write("ok"); print("works (the only writable host folder)")
print("model files:", sorted(os.listdir("/models/nli-minilm2")))
