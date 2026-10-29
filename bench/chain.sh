#!/data/data/com.termux/files/usr/bin/sh
# Sequential, resumable benchmark chain. Each step resumes from its .partial.jsonl if interrupted.
cd ~/bench
termux-wake-lock
R=results
ST=$R/stage1_sdi.json
L=~/models/LFM2-350M-Q4_0.gguf
Q=~/models/qwen2.5-0.5b-q4.gguf
G=~/models/gemma-3-270m-it-Q4_0.gguf
step() { echo "[$(date +%H:%M:%S)] start $1" >> $R/chain.log; shift; "$@" >> $R/chain.log 2>&1; echo "[$(date +%H:%M:%S)] end rc=$?" >> $R/chain.log; }
step "LFM2 full verdicts"     python bench.py llm      --dataset sdi-ch4-8 --model $L --out $R/b5_llm_LFM2.json
step "LFM2 pipeline judge"    python bench.py pipeline --dataset sdi-ch4-8 --model $L --stage1 $ST --out $R/b6_pipe_LFM2.json
step "Qwen full verdicts"     python bench.py llm      --dataset sdi-ch4-8 --model $Q --out $R/b5_llm_Qwen.json
step "Qwen pipeline judge"    python bench.py pipeline --dataset sdi-ch4-8 --model $Q --stage1 $ST --out $R/b6_pipe_Qwen.json
step "Gemma full verdicts"    python bench.py llm      --dataset sdi-ch4-8 --model $G --out $R/b5_llm_Gemma.json
step "LFM2 narrator sample"   python bench.py narrate  --dataset sdi-ch4-8 --model $L --stage1 $ST --out $R/b7_narr_LFM2.json
step "Qwen narrator sample"   python bench.py narrate  --dataset sdi-ch4-8 --model $Q --stage1 $ST --out $R/b7_narr_Qwen.json
echo "ALL DONE" >> $R/chain.log
termux-wake-unlock
