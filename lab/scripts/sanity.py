import sys
sys.path.insert(0, "/work/bench")
import ekm
for name, path in [("phone GGUF Q4_0 (B3-04)", "/work/bench/results/stage1_sdi.json"), ("Mac fp32 PyTorch", "/work/out/stage1_mac_ds2.json")]:
    m = ekm.metrics([(p["lab"], p["stage1"]) for p in ekm.loco(ekm.load(path))])
    print(f"{name:28} exact {m['exact']}  F1 {m['flag_f1']}  flagged/named {m['contradicted_flagged']} / {m['contradicted_named']}  false {m['false_contradicted']}")
