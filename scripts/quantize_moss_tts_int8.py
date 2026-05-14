#!/usr/bin/env python3
"""Quantize the MOSS TTS ONNX models to dynamic int8 weights.

The MOSS TTS component stores several ONNX graph files next to shared external
data files. This script quantizes the TTS ONNX files, rewrites their external
data metadata, and can replace the current model directory in-place after a
successful validation pass.
"""

from __future__ import annotations

import argparse
import json
import shutil
import sys
import tempfile
from datetime import datetime
from pathlib import Path
from typing import Any

import onnx
import onnxruntime as ort
from onnxruntime.quantization import QuantType, quantize_dynamic


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_TTS_DIR = (
    REPO_ROOT
    / "models"
    / "tts"
    / "moss-tts-nano-100m-onnx"
    / "MOSS-TTS-Nano-100M-ONNX"
)
TTS_META = "tts_browser_onnx_meta.json"
DEFAULT_OPS = ["MatMul"]


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="Quantize MOSS TTS ONNX graph files to dynamic int8.",
    )
    parser.add_argument(
        "--model-dir",
        type=Path,
        default=DEFAULT_TTS_DIR,
        help=f"MOSS-TTS-Nano-100M-ONNX directory. Default: {DEFAULT_TTS_DIR}",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        help="Write quantized files to this directory instead of replacing model-dir.",
    )
    parser.add_argument(
        "--in-place",
        action="store_true",
        help="Replace model-dir files after quantization succeeds.",
    )
    parser.add_argument(
        "--backup-dir",
        type=Path,
        help="Backup directory used with --in-place. Defaults to model-dir.backup-int8-<timestamp>.",
    )
    parser.add_argument(
        "--models",
        nargs="+",
        help=(
            "Specific TTS file keys from tts_browser_onnx_meta.json to quantize. "
            "Defaults to all keys in the metadata files map."
        ),
    )
    parser.add_argument(
        "--op-types",
        nargs="+",
        default=DEFAULT_OPS,
        help=f"ONNX operator types to quantize. Default: {' '.join(DEFAULT_OPS)}",
    )
    parser.add_argument(
        "--per-channel",
        action=argparse.BooleanOptionalAction,
        default=True,
        help="Enable per-channel weight quantization. Default: enabled.",
    )
    parser.add_argument(
        "--reduce-range",
        action="store_true",
        help="Use 7-bit weight range for older CPUs without VNNI.",
    )
    parser.add_argument(
        "--skip-session-check",
        action="store_true",
        help="Skip ONNX Runtime session creation checks for quantized files.",
    )
    return parser.parse_args()


def read_json(path: Path) -> dict[str, Any]:
    """Read a JSON object from disk."""
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def write_json(path: Path, data: dict[str, Any]) -> None:
    """Write a JSON object with stable formatting."""
    with path.open("w", encoding="utf-8") as handle:
        json.dump(data, handle, indent=2, ensure_ascii=False)
        handle.write("\n")


def copy_tree_contents(src: Path, dst: Path) -> None:
    """Copy a directory's contents into an existing or new destination."""
    dst.mkdir(parents=True, exist_ok=True)
    for child in src.iterdir():
        target = dst / child.name
        if child.is_dir():
            if child.name == ".git":
                continue
            shutil.copytree(child, target, dirs_exist_ok=True)
        else:
            shutil.copy2(child, target)


def file_size(path: Path) -> int:
    """Return file size or zero when the path does not exist."""
    return path.stat().st_size if path.exists() else 0


def validate_session(model_path: Path) -> None:
    """Ensure ONNX and ONNX Runtime can load the quantized model."""
    onnx.load(str(model_path), load_external_data=True)
    providers = ["CPUExecutionProvider"]
    ort.InferenceSession(str(model_path), providers=providers)


def quantize_one(
    source: Path,
    output: Path,
    op_types: list[str],
    per_channel: bool,
    reduce_range: bool,
) -> None:
    """Quantize one ONNX file to a temporary output path."""
    output.parent.mkdir(parents=True, exist_ok=True)
    quantize_dynamic(
        source,
        output,
        op_types_to_quantize=op_types,
        per_channel=per_channel,
        reduce_range=reduce_range,
        weight_type=QuantType.QInt8,
        use_external_data_format=True,
        extra_options={"WeightSymmetric": True},
    )


def backup_current_files(model_dir: Path, backup_dir: Path, meta: dict[str, Any]) -> None:
    """Back up files that may be replaced or no longer referenced after quantization."""
    backup_dir.mkdir(parents=True, exist_ok=False)
    files = set(meta.get("files", {}).values())
    files.add(TTS_META)
    for external_files in meta.get("external_data_files", {}).values():
        files.update(external_files)

    for raw_name in sorted(files):
        source = model_dir / raw_name
        if source.is_file():
            target = backup_dir / raw_name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)


def replace_quantized_files(model_dir: Path, work_dir: Path, meta: dict[str, Any]) -> None:
    """Replace model-dir ONNX files, their .data files, and metadata."""
    for raw_name in meta["files"].values():
        source_onnx = work_dir / raw_name
        source_data = work_dir / f"{raw_name}.data"
        shutil.copy2(source_onnx, model_dir / raw_name)
        if source_data.is_file():
            shutil.copy2(source_data, model_dir / source_data.name)
    shutil.copy2(work_dir / TTS_META, model_dir / TTS_META)


def remove_unreferenced_external_data(model_dir: Path, old_meta: dict[str, Any], new_meta: dict[str, Any]) -> None:
    """Move old shared external data out of the active model directory when unused."""
    new_external = {
        name
        for external_files in new_meta.get("external_data_files", {}).values()
        for name in external_files
    }
    for external_files in old_meta.get("external_data_files", {}).values():
        for raw_name in external_files:
            if raw_name in new_external:
                continue
            path = model_dir / raw_name
            if path.is_file():
                path.unlink()


def main() -> int:
    """Run MOSS TTS int8 quantization."""
    args = parse_args()
    model_dir = args.model_dir.resolve()
    meta_path = model_dir / TTS_META
    if not meta_path.is_file():
        print(f"error: metadata file not found: {meta_path}", file=sys.stderr)
        return 2

    if args.output_dir and args.in_place:
        print("error: use either --output-dir or --in-place, not both", file=sys.stderr)
        return 2
    if not args.output_dir and not args.in_place:
        print("error: choose --output-dir or --in-place", file=sys.stderr)
        return 2

    meta = read_json(meta_path)
    files = meta.get("files", {})
    if not isinstance(files, dict) or not files:
        print(f"error: {meta_path} does not contain a non-empty files map", file=sys.stderr)
        return 2

    selected_keys = args.models or list(files.keys())
    unknown_keys = [key for key in selected_keys if key not in files]
    if unknown_keys:
        print(f"error: unknown model keys: {', '.join(unknown_keys)}", file=sys.stderr)
        return 2

    output_dir = args.output_dir.resolve() if args.output_dir else None
    with tempfile.TemporaryDirectory(prefix="moss-tts-int8-", dir=str(model_dir.parent)) as tmp:
        work_dir = Path(tmp)
        copy_tree_contents(model_dir, work_dir)
        output_root = output_dir or work_dir
        if output_dir:
            copy_tree_contents(model_dir, output_root)

        new_meta = read_json(output_root / TTS_META)
        external_data_files = dict(new_meta.get("external_data_files", {}))

        print(f"model dir: {model_dir}")
        print(f"output dir: {output_root}")
        print(f"models: {', '.join(selected_keys)}")
        print(f"ops: {', '.join(args.op_types)}")

        for key in selected_keys:
            raw_name = files[key]
            source = model_dir / raw_name
            output = output_root / raw_name
            if not source.is_file():
                print(f"error: missing source model: {source}", file=sys.stderr)
                return 2

            before_size = file_size(source)
            for stale_data in output_root.glob(f"{raw_name}.data*"):
                stale_data.unlink()

            print(f"quantizing {key}: {raw_name}")
            quantize_one(
                source=source,
                output=output,
                op_types=args.op_types,
                per_channel=args.per_channel,
                reduce_range=args.reduce_range,
            )

            data_name = f"{raw_name}.data"
            if (output_root / data_name).is_file():
                external_data_files[raw_name] = [data_name]
            else:
                external_data_files.pop(raw_name, None)

            after_size = file_size(output) + file_size(output_root / data_name)
            print(f"  size: {before_size / 1024 / 1024:.1f} MiB -> {after_size / 1024 / 1024:.1f} MiB")

            if not args.skip_session_check:
                validate_session(output)
                print("  validation: onnx + ort session ok")

        new_meta["external_data_files"] = external_data_files
        write_json(output_root / TTS_META, new_meta)

        if args.in_place:
            timestamp = datetime.now().strftime("%Y%m%d-%H%M%S")
            backup_dir = (args.backup_dir or model_dir.with_name(f"{model_dir.name}.backup-int8-{timestamp}")).resolve()
            backup_current_files(model_dir, backup_dir, meta)
            replace_quantized_files(model_dir, output_root, new_meta)
            remove_unreferenced_external_data(model_dir, meta, new_meta)
            print(f"backup dir: {backup_dir}")
            print("in-place replacement complete")
        else:
            print("quantized copy complete")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
