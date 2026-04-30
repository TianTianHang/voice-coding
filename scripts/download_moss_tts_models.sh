#!/usr/bin/env bash
set -euo pipefail

MODEL_ROOT="${1:-models/moss-tts}"
TTS_REPO="OpenMOSS-Team/MOSS-TTS-Nano-100M-ONNX"
CODEC_REPO="OpenMOSS-Team/MOSS-Audio-Tokenizer-Nano-ONNX"

TTS_DIR="${MODEL_ROOT}/MOSS-TTS-Nano-100M-ONNX"
CODEC_DIR="${MODEL_ROOT}/MOSS-Audio-Tokenizer-Nano-ONNX"

if ! command -v git >/dev/null 2>&1; then
  echo "Error: git is required but not installed."
  exit 1
fi

echo "Downloading MOSS TTS ONNX models into ${MODEL_ROOT} ..."
mkdir -p "${MODEL_ROOT}"

download_repo() {
  local repo="$1"
  local target_dir="$2"
  shift 2
  local files=("$@")
  local temp_clone="${target_dir}.tmp"

  rm -rf "${temp_clone}" "${target_dir}"

  echo "Cloning https://huggingface.co/${repo} (sparse checkout)..."
  git clone --depth 1 --no-checkout "https://huggingface.co/${repo}" "${temp_clone}"
  git -C "${temp_clone}" sparse-checkout init --no-cone
  git -C "${temp_clone}" sparse-checkout set "${files[@]}"
  git -C "${temp_clone}" checkout

  if git -C "${temp_clone}" lfs ls-files >/dev/null 2>&1; then
    echo "Pulling LFS objects for ${repo} ..."
    local include_csv
    include_csv="$(IFS=, ; echo "${files[*]}")"
    git -C "${temp_clone}" lfs pull --include="${include_csv}" --exclude=""
  fi

  mv "${temp_clone}" "${target_dir}"
}

download_repo "${TTS_REPO}" "${TTS_DIR}" \
  "browser_poc_manifest.json" \
  "tts_browser_onnx_meta.json" \
  "tokenizer.model" \
  "moss_tts_prefill.onnx" \
  "moss_tts_decode_step.onnx" \
  "moss_tts_global_shared.data" \
  "moss_tts_local_decoder.onnx" \
  "moss_tts_local_cached_step.onnx" \
  "moss_tts_local_fixed_sampled_frame.onnx" \
  "moss_tts_local_shared.data"

download_repo "${CODEC_REPO}" "${CODEC_DIR}" \
  "codec_browser_onnx_meta.json" \
  "moss_audio_tokenizer_encode.onnx" \
  "moss_audio_tokenizer_encode.data" \
  "moss_audio_tokenizer_decode_full.onnx" \
  "moss_audio_tokenizer_decode_step.onnx" \
  "moss_audio_tokenizer_decode_shared.data"

echo "Verifying required files ..."
REQUIRED_FILES=(
  "${TTS_DIR}/browser_poc_manifest.json"
  "${TTS_DIR}/tts_browser_onnx_meta.json"
  "${TTS_DIR}/tokenizer.model"
  "${TTS_DIR}/moss_tts_prefill.onnx"
  "${TTS_DIR}/moss_tts_decode_step.onnx"
  "${TTS_DIR}/moss_tts_global_shared.data"
  "${TTS_DIR}/moss_tts_local_decoder.onnx"
  "${TTS_DIR}/moss_tts_local_cached_step.onnx"
  "${TTS_DIR}/moss_tts_local_fixed_sampled_frame.onnx"
  "${TTS_DIR}/moss_tts_local_shared.data"
  "${CODEC_DIR}/codec_browser_onnx_meta.json"
  "${CODEC_DIR}/moss_audio_tokenizer_encode.onnx"
  "${CODEC_DIR}/moss_audio_tokenizer_decode_full.onnx"
  "${CODEC_DIR}/moss_audio_tokenizer_decode_step.onnx"
)

MISSING=0
for f in "${REQUIRED_FILES[@]}"; do
  if [ ! -f "${f}" ]; then
    echo "WARNING: Missing required file: ${f}"
    MISSING=1
  fi
done

if [ "${MISSING}" -ne 0 ]; then
  echo
  echo "Some files are missing. Check model repos:"
  echo "  https://huggingface.co/${TTS_REPO}"
  echo "  https://huggingface.co/${CODEC_REPO}"
  exit 1
fi

echo "All required files verified."
echo
echo "Expected layout for manifest-relative linking:"
echo "  ${MODEL_ROOT}/"
echo "    MOSS-TTS-Nano-100M-ONNX/"
echo "    MOSS-Audio-Tokenizer-Nano-ONNX/"
echo
echo "Done."
