# Sandbox image for offline experiments (Python, scikit-learn, PyTorch CPU).
# Only the BUILD uses the network. Runs use --network none (see run.sh).
FROM python:3.12-slim
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 PIP_NO_CACHE_DIR=1 PIP_DISABLE_PIP_VERSION_CHECK=1 \
    HF_HUB_DISABLE_TELEMETRY=1
COPY requirements.txt /tmp/requirements.txt
# wheels only: a package can never run its own setup script during install
RUN pip install --only-binary=:all: -r /tmp/requirements.txt && rm /tmp/requirements.txt
# newer Hugging Face client (the pinned one stalls on the xet storage used for large files), still wheels only
RUN pip install --only-binary=:all: "huggingface-hub==0.34.4" "hf-xet==1.1.5"
# model weights baked in: safetensors only (no pickle files that could execute code); timeouts so a stall fails
RUN HF_HUB_DOWNLOAD_TIMEOUT=60 HF_HUB_ETAG_TIMEOUT=30 python -c "from huggingface_hub import snapshot_download; snapshot_download('cross-encoder/nli-MiniLM2-L6-H768', local_dir='/models/nli-minilm2', allow_patterns=['*.json','*.txt','model.safetensors'])" \
 && ls -la /models/nli-minilm2
# sentence embedding model (same one the phone runs as GGUF), safetensors only
RUN HF_HUB_DOWNLOAD_TIMEOUT=60 HF_HUB_ETAG_TIMEOUT=30 python -c "from huggingface_hub import snapshot_download; snapshot_download('sentence-transformers/all-MiniLM-L6-v2', local_dir='/models/minilm', allow_patterns=['*.json','*.txt','model.safetensors'])" \
 && ls -la /models/minilm
RUN useradd --uid 10001 --no-create-home --shell /usr/sbin/nologin runner
USER 10001
WORKDIR /work
