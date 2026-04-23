#!/bin/bash
# Test runner script for stt-qwen3 with ORT environment setup

export ORT_DYLIB_PATH=/nix/store/mgzpl0scz1my17vwv9av0nf56djd455a-onnxruntime-1.24.4/lib/libonnxruntime.so

echo "================================"
echo "ASR Engine Test Suite"
echo "================================"
echo ""

echo "1. Running existing unit tests (fast)..."
cargo test --lib --quiet
if [ $? -eq 0 ]; then
    echo "✅ Unit tests passed"
else
    echo "❌ Unit tests failed"
    exit 1
fi
echo ""

echo "2. Running engine tests (slower, requires model)..."
echo "   ⚠️  Note: Running with --test-threads=1 to avoid memory issues"
cargo test --test engine_test -- --test-threads=1
if [ $? -eq 0 ]; then
    echo "✅ Engine tests passed"
else
    echo "❌ Engine tests failed"
    exit 1
fi
echo ""

echo "3. Running boundary tests (slower, requires model)..."
echo "   ⚠️  Note: Running with --test-threads=1 to avoid memory issues"
cargo test --test boundary_test -- --test-threads=1
if [ $? -eq 0 ]; then
    echo "✅ Boundary tests passed"
else
    echo "❌ Boundary tests failed"
    exit 1
fi
echo ""

echo "================================"
echo "All tests passed! ✅"
echo "================================"
