#!/usr/bin/env bash
set -euo pipefail

MODEL_DIR="${1:-models}"
REPO="andrewleech/qwen3-asr-0.6b-onnx"
export GIT_LFS_SKIP_SMUDGE=0
echo "Downloading Qwen3-ASR-0.6B int4 ONNX models to ${MODEL_DIR}..."

mkdir -p "${MODEL_DIR}/onnx_models"

if ! command -v git &> /dev/null; then
    echo "Error: git is required but not installed"
    exit 1
fi

echo "Cloning model repository (standard shallow checkout)..."
rm -rf "${MODEL_DIR}/temp_clone"
git clone --depth 1 --no-checkout "https://huggingface.co/${REPO}" "${MODEL_DIR}/temp_clone"
git -C "${MODEL_DIR}/temp_clone" sparse-checkout init --no-cone
git -C "${MODEL_DIR}/temp_clone" sparse-checkout set \
    encoder.int4.onnx \
    decoder_init.int4.onnx \
    decoder_step.int4.onnx \
    decoder_weights.int4.data \
    embed_tokens.bin \
    config.json \
    tokenizer.json
git -C "${MODEL_DIR}/temp_clone" checkout
git -C "${MODEL_DIR}/temp_clone" lfs pull --include="encoder.int4.onnx,decoder_init.int4.onnx,decoder_step.int4.onnx,decoder_weights.int4.data,embed_tokens.bin,config.json,tokenizer.json" --exclude=""

echo "Copying model files..."
cp "${MODEL_DIR}/temp_clone/tokenizer.json" "${MODEL_DIR}/" 2>/dev/null || true
cp "${MODEL_DIR}/temp_clone/config.json" "${MODEL_DIR}/" 2>/dev/null || true
cp "${MODEL_DIR}/temp_clone/embed_tokens.bin" "${MODEL_DIR}/" 2>/dev/null || true

for file in encoder.int4.onnx decoder_init.int4.onnx decoder_step.int4.onnx decoder_weights.int4.data; do
    cp "${MODEL_DIR}/temp_clone/${file}" "${MODEL_DIR}/onnx_models/" 2>/dev/null || true
done

rm -rf "${MODEL_DIR}/temp_clone"

echo "Verifying model files..."
REQUIRED_FILES=(
    "${MODEL_DIR}/tokenizer.json"
    "${MODEL_DIR}/config.json"
    "${MODEL_DIR}/embed_tokens.bin"
    "${MODEL_DIR}/onnx_models/encoder.int4.onnx"
    "${MODEL_DIR}/onnx_models/decoder_init.int4.onnx"
    "${MODEL_DIR}/onnx_models/decoder_step.int4.onnx"
    "${MODEL_DIR}/onnx_models/decoder_weights.int4.data"
)

MISSING=0
for f in "${REQUIRED_FILES[@]}"; do
    if [ ! -f "$f" ]; then
        echo "WARNING: Missing required file: $f"
        MISSING=1
    fi
done

for decoder in "${MODEL_DIR}/onnx_models/decoder_init.int4.onnx" "${MODEL_DIR}/onnx_models/decoder_step.int4.onnx"; do
    if [ -f "$decoder" ]; then
        echo "Found model: $decoder"
    fi
done

if [ $MISSING -eq 0 ]; then
    echo "All model files verified successfully!"
else
    echo ""
    echo "Some files are missing. You may need to manually download from:"
    echo "  https://huggingface.co/${REPO}"
    exit 1
fi

echo ""
echo "Model download complete. Set environment variable:"
echo "  export STT_MODEL_DIR=\"${MODEL_DIR}\""
