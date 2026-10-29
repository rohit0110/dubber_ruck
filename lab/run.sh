#!/bin/sh
# Run a script from lab/scripts inside the sandbox.  Usage: lab/run.sh scripts/combiner.py [args]
#  - no network, read-only root filesystem, no capabilities, no privilege escalation, non-root user
#  - memory and CPU capped
#  - only data/, tests/, bench/ and lab/scripts are visible, all read-only
#  - the only writable host folder is lab/out
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
mkdir -p "$root/lab/out"
exec docker run --rm --network none --read-only --tmpfs /tmp:rw,size=512m \
  --cap-drop ALL --security-opt no-new-privileges --pids-limit 256 --memory 4g --cpus 4 \
  --user 10001:10001 -e HOME=/tmp -e HF_HUB_OFFLINE=1 -e TRANSFORMERS_OFFLINE=1 -e PYTHONDONTWRITEBYTECODE=1 \
  -v "$root/data:/work/data:ro" -v "$root/tests:/work/tests:ro" -v "$root/bench:/work/bench:ro" \
  -v "$root/lab/scripts:/work/scripts:ro" -v "$root/lab/out:/work/out:rw" \
  duck-lab:1 python "$@"
