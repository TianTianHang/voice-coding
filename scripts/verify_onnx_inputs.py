#!/usr/bin/env python3
"""Verify ONNX model input/output specifications.

This script loads all ONNX models from models/onnx_models/ and prints
their input/output specifications for debugging and documentation purposes.
"""

import os
from pathlib import Path
import onnx
from onnx import helper, numpy_helper
import json


def get_tensor_type_str(type_int):
    """Convert ONNX tensor type int to readable string."""
    types = {
        0: "UNDEFINED",
        1: "FLOAT",
        2: "UINT8",
        3: "INT8",
        4: "UINT16",
        5: "INT16",
        6: "INT32",
        7: "INT64",
        8: "STRING",
        9: "BOOL",
        10: "FLOAT16",
        11: "DOUBLE",
        12: "UINT32",
        13: "UINT64",
        14: "COMPLEX64",
        15: "COMPLEX128",
        16: "BFLOAT16",
    }
    return types.get(type_int, f"UNKNOWN({type_int})")


def analyze_model(model_path: Path) -> dict | None:
    """Analyze an ONNX model and extract input/output info."""
    print(f"\n{'=' * 80}")
    print(f"Loading model: {model_path.name}")
    print(f"{'=' * 80}")

    try:
        model = onnx.load(str(model_path))
    except Exception as e:
        print(f"Error loading model: {e}")
        return None

    # Check model version and producer
    print(f"\nModel Info:")
    print(f"  IR Version: {model.ir_version}")
    print(f"  Producer: {model.producer_name}")
    print(f"  Graph Name: {model.graph.name}")

    # Extract input information
    inputs = []
    print(f"\nInputs ({len(model.graph.input)}):")
    for i, input_tensor in enumerate(model.graph.input):
        name = input_tensor.name
        type_str = get_tensor_type_str(input_tensor.type.tensor_type.elem_type)

        # Get shape
        shape = []
        if input_tensor.type.tensor_type.HasField("shape"):
            for dim in input_tensor.type.tensor_type.shape.dim:
                if dim.HasField("dim_value"):
                    shape.append(dim.dim_value)
                elif dim.HasField("dim_param"):
                    shape.append(dim.dim_param)
                else:
                    shape.append("?")

        input_info = {"index": i, "name": name, "type": type_str, "shape": shape}
        inputs.append(input_info)

        print(f"  [{i}] {name}")
        print(f"      Type: {type_str}")
        print(f"      Shape: {shape}")

    # Extract output information
    outputs = []
    print(f"\nOutputs ({len(model.graph.output)}):")
    for i, output_tensor in enumerate(model.graph.output):
        name = output_tensor.name
        type_str = get_tensor_type_str(output_tensor.type.tensor_type.elem_type)

        # Get shape
        shape = []
        if output_tensor.type.tensor_type.HasField("shape"):
            for dim in output_tensor.type.tensor_type.shape.dim:
                if dim.HasField("dim_value"):
                    shape.append(dim.dim_value)
                elif dim.HasField("dim_param"):
                    shape.append(dim.dim_param)
                else:
                    shape.append("?")

        output_info = {"index": i, "name": name, "type": type_str, "shape": shape}
        outputs.append(output_info)

        print(f"  [{i}] {name}")
        print(f"      Type: {type_str}")
        print(f"      Shape: {shape}")

    return {
        "model_name": model_path.name,
        "graph_name": model.graph.name,
        "inputs": inputs,
        "outputs": outputs,
    }


def generate_markdown_report(all_models_info: dict, output_path: Path):
    """Generate a Markdown report with all model information."""

    md_content = [
        "# ONNX Models Input/Output Specification Report\n",
        "This document describes the input and output specifications for all ONNX models\n",
        "used in the voice-coding project.\n",
        "\n",
    ]

    for model_name, info in all_models_info.items():
        if info is None:
            continue

        md_content.extend(
            [
                f"## {model_name}\n",
                f"\n",
                f"**Graph Name**: `{info['graph_name']}`\n",
                f"\n",
                f"### Inputs\n",
                f"\n",
                f"| Index | Name | Type | Shape |\n",
                f"|-------|------|------|-------|\n",
            ]
        )

        for inp in info["inputs"]:
            shape_str = str(inp["shape"]).replace("'", "")
            md_content.append(
                f"| {inp['index']} | `{inp['name']}` | {inp['type']} | {shape_str} |\n"
            )

        md_content.extend(
            [
                f"\n",
                f"### Outputs\n",
                f"\n",
                f"| Index | Name | Type | Shape |\n",
                f"|-------|------|------|-------|\n",
            ]
        )

        for out in info["outputs"]:
            shape_str = str(out["shape"]).replace("'", "")
            md_content.append(
                f"| {out['index']} | `{out['name']}` | {out['type']} | {shape_str} |\n"
            )

        md_content.append("\n---\n\n")

    # Add comparison with Rust code
    md_content.extend(
        [
            "## Comparison with Rust Implementation\n",
            "\n",
            "### encoder_conv.onnx\n",
            "- **Rust usage**: `src-tauri/stt-qwen3/src/encoder.rs:81`\n",
            "- **Input name**: `padded_mel_chunks`\n",
            "- **Expected input**: 4D tensor `[batch, n_chunks, n_mels, chunk_len]`\n",
            "\n",
            "### encoder_transformer.onnx\n",
            "- **Rust usage**: `src-tauri/stt-qwen3/src/encoder.rs:150-156`\n",
            "- **Input**: Output from encoder_conv\n",
            "- **Outputs**: Encoder representations for decoder\n",
            "\n",
            "### decoder_init.int8.onnx\n",
            "- **Rust usage**: `src-tauri/stt-qwen3/src/decoder.rs:84-90`\n",
            "- **Input name**: `input_embeds`\n",
            "- **Expected inputs**: \n",
            "  - `input_embeds`: 3D tensor `[1, seq_len, hidden_size]`\n",
            "- **Outputs**: \n",
            "  - `logits`: Token prediction logits\n",
            "  - `present_keys`: KV-cache keys\n",
            "  - `present_values`: KV-cache values\n",
            "\n",
            "### decoder_step.int8.onnx\n",
            "- **Rust usage**: `src-tauri/stt-qwen3/src/decoder.rs:216-224`\n",
            "- **Expected inputs**:\n",
            "  - `input_embeds`: Single token embedding\n",
            "  - KV-cache from previous steps\n",
            "- **Outputs**: \n",
            "  - `logits`: Token prediction logits\n",
            "  - Updated KV-cache\n",
        ]
    )

    with open(output_path, "w") as f:
        f.writelines(md_content)

    print(f"\n{'=' * 80}")
    print(f"Markdown report saved to: {output_path}")
    print(f"{'=' * 80}")


def compare_with_rust(all_models_info: dict):
    """Compare model specifications with Rust code expectations."""
    print(f"\n{'=' * 80}")
    print("Comparison with Rust Implementation")
    print(f"{'=' * 80}\n")

    # Check encoder_conv
    if "encoder_conv.onnx" in all_models_info:
        info = all_models_info["encoder_conv.onnx"]
        input_names = [inp["name"] for inp in info["inputs"]]
        print("✓ encoder_conv.onnx")
        print(f"  Input names: {input_names}")
        print(f"  Rust expects: 'padded_mel_chunks'")
        if "padded_mel_chunks" in input_names:
            print("  ✓ Matches!")
        else:
            print("  ✗ WARNING: Name mismatch!")
        print()

    # Check decoder_init
    if "decoder_init.int8.onnx" in all_models_info:
        info = all_models_info["decoder_init.int8.onnx"]
        input_names = [inp["name"] for inp in info["inputs"]]
        print("✓ decoder_init.int8.onnx")
        print(f"  Input names: {input_names}")
        print(f"  Rust expects: 'input_embeds'")
        if "input_embeds" in input_names:
            print("  ✓ Matches!")
        else:
            print("  ✗ WARNING: Name mismatch!")

        output_names = [out["name"] for out in info["outputs"]]
        print(f"  Output names: {output_names}")
        print(f"  Rust expects: logits, present_keys, present_values")
        print()

    # Check decoder_step
    if "decoder_step.int8.onnx" in all_models_info:
        info = all_models_info["decoder_step.int8.onnx"]
        input_names = [inp["name"] for inp in info["inputs"]]
        print("✓ decoder_step.int8.onnx")
        print(f"  Input names: {input_names}")
        print(f"  Rust expects: input_embeds + cache inputs")
        print()

    # Check encoder_transformer
    if "encoder_transformer.onnx" in all_models_info:
        info = all_models_info["encoder_transformer.onnx"]
        input_names = [inp["name"] for inp in info["inputs"]]
        print("✓ encoder_transformer.onnx")
        print(f"  Input names: {input_names}")
        print()


def main():
    # Set paths
    project_root = Path(__file__).parent.parent
    models_dir = project_root / "models" / "onnx_models"
    docs_dir = project_root / "docs"
    report_path = docs_dir / "model_inputs_report.md"

    # Create docs directory if it doesn't exist
    docs_dir.mkdir(exist_ok=True)

    # Find all ONNX files
    onnx_files = sorted(models_dir.glob("*.onnx"))

    if not onnx_files:
        print(f"No ONNX files found in {models_dir}")
        return

    print(f"Found {len(onnx_files)} ONNX model files:")
    for f in onnx_files:
        print(f"  - {f.name}")

    # Analyze all models
    all_models_info = {}
    for model_path in onnx_files:
        info = analyze_model(model_path)
        all_models_info[model_path.name] = info

    # Compare with Rust code
    compare_with_rust(all_models_info)

    # Generate markdown report
    generate_markdown_report(all_models_info, report_path)

    # Also save JSON for programmatic access
    json_path = docs_dir / "model_inputs_spec.json"
    with open(json_path, "w") as f:
        json.dump(all_models_info, f, indent=2)
    print(f"\nJSON spec saved to: {json_path}")

    print(f"\n{'=' * 80}")
    print("Analysis complete!")
    print(f"{'=' * 80}")


if __name__ == "__main__":
    main()
