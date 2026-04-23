#!/usr/bin/env bash
set -euo pipefail

MODEL_DIR="${1:-models}"
REPO="jasonzhang76/Qwen3-ASR-0.6B-ONNX-CPU"
export GIT_LFS_SKIP_SMUDGE=0
echo "Downloading Qwen3-ASR-0.6B ONNX models to ${MODEL_DIR}..."

mkdir -p "${MODEL_DIR}/onnx_models"

if ! command -v git &> /dev/null; then
    echo "Error: git is required but not installed"
    exit 1
fi

if [ ! -d "${MODEL_DIR}/.git" ]; then
    echo "Cloning model repository (this may take a while ~2.5GB)..."
    git clone --depth 1 "https://huggingface.co/${REPO}" "${MODEL_DIR}/temp_clone"
    
    echo "Copying model files..."
    cp "${MODEL_DIR}/temp_clone/tokenizer.json" "${MODEL_DIR}/" 2>/dev/null || true
    
    if [ -d "${MODEL_DIR}/temp_clone/onnx_models" ]; then
        cp "${MODEL_DIR}/temp_clone/onnx_models/"*.onnx "${MODEL_DIR}/onnx_models/" 2>/dev/null || true
        cp "${MODEL_DIR}/temp_clone/onnx_models/"*.onnx.data "${MODEL_DIR}/onnx_models/" 2>/dev/null || true
    fi

    find "${MODEL_DIR}/temp_clone" -name "embed_tokens*" -exec cp {} "${MODEL_DIR}/" \; 2>/dev/null || true
    
    rm -rf "${MODEL_DIR}/temp_clone"
else
    echo "Model repository already exists, pulling latest..."
    git -C "${MODEL_DIR}" pull || true
fi

echo "Verifying model files..."
REQUIRED_FILES=(
    "${MODEL_DIR}/tokenizer.json"
    "${MODEL_DIR}/onnx_models/encoder_conv.onnx"
    "${MODEL_DIR}/onnx_models/encoder_transformer.onnx"
    "${MODEL_DIR}/onnx_models/encoder_conv.onnx.data"
    "${MODEL_DIR}/onnx_models/encoder_transformer.onnx.data"
    "${MODEL_DIR}/onnx_models/decoder_init.int8.onnx"
    "${MODEL_DIR}/onnx_models/decoder_init.onnx"
)

MISSING=0
for f in "${REQUIRED_FILES[@]}"; do
    if [ ! -f "$f" ]; then
        echo "WARNING: Missing required file: $f"
        MISSING=1
    fi
done

DECODER_FOUND=0
for decoder in "${MODEL_DIR}/onnx_models/decoder_init.int8.onnx" "${MODEL_DIR}/onnx_models/decoder_init.onnx"; do
    if [ -f "$decoder" ]; then
        echo "Found decoder_init: $decoder"
        DECODER_FOUND=1
        break
    fi
done

if [ $DECODER_FOUND -eq 0 ]; then
    echo "WARNING: No decoder_init model found"
    MISSING=1
fi

DECODER_STEP_FOUND=0
for decoder in "${MODEL_DIR}/onnx_models/decoder_step.int8.onnx" "${MODEL_DIR}/onnx_models/decoder_step.onnx"; do
    if [ -f "$decoder" ]; then
        echo "Found decoder_step: $decoder"
        DECODER_STEP_FOUND=1
        break
    fi
done

if [ $DECODER_STEP_FOUND -eq 0 ]; then
    echo "WARNING: No decoder_step model found"
    MISSING=1
fi

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
