#!/usr/bin/env python3
"""Download LibriSpeech test-other samples from HuggingFace for Qwen3 WER tests."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Download LibriSpeech other/test samples and write a JSONL manifest."
    )
    parser.add_argument("--dataset", default="openslr/librispeech_asr")
    parser.add_argument("--config", default="other")
    parser.add_argument("--split", default="test")
    parser.add_argument("--n-samples", type=int, default=200)
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("test_audio/librispeech-other-200"),
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path("test_audio/librispeech-other-200.jsonl"),
    )
    parser.add_argument(
        "--no-streaming",
        action="store_true",
        help="Disable HuggingFace streaming mode and use normal dataset loading.",
    )
    parser.add_argument(
        "--parquet",
        type=Path,
        help="Read samples from a local HuggingFace LibriSpeech parquet file instead of downloading.",
    )
    return parser.parse_args()


def import_datasets() -> Any:
    try:
        from datasets import Audio, load_dataset
    except ImportError as exc:
        raise SystemExit(
            "Missing dependency: datasets. Install with `.venv/bin/python -m pip install datasets`."
        ) from exc

    return Audio, load_dataset


def iter_parquet_samples(parquet_path: Path) -> Any:
    try:
        import pyarrow.parquet as pq
    except ImportError as exc:
        raise SystemExit(
            "Missing dependency: pyarrow. Install with `.venv/bin/python -m pip install pyarrow`."
        ) from exc

    if not parquet_path.exists():
        raise SystemExit(f"Parquet file not found: {parquet_path}")

    table = pq.read_table(parquet_path)
    for row in table.to_pylist():
        yield row


def write_audio_bytes(path: Path, audio_bytes: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(audio_bytes)


def sample_id(example: dict[str, Any], index: int) -> str:
    speaker = example.get("speaker_id", "speaker")
    chapter = example.get("chapter_id", "chapter")
    sample = example.get("id", f"sample-{index:04d}")
    return f"{speaker}-{chapter}-{sample}"


def main() -> None:
    args = parse_args()
    if args.n_samples <= 0:
        raise SystemExit("--n-samples must be greater than 0")

    args.output_dir.mkdir(parents=True, exist_ok=True)
    args.manifest.parent.mkdir(parents=True, exist_ok=True)

    if args.parquet:
        print(f"Loading local parquet {args.parquet} for {args.n_samples} samples...")
        dataset = iter_parquet_samples(args.parquet)
    else:
        Audio, load_dataset = import_datasets()
        print(
            f"Loading {args.dataset} config={args.config} split={args.split} "
            f"for {args.n_samples} samples..."
        )
        split = f"{args.split}[:{args.n_samples}]" if args.no_streaming else args.split
        dataset = load_dataset(
            args.dataset,
            args.config,
            split=split,
            streaming=not args.no_streaming,
            trust_remote_code=False,
        )
        dataset = dataset.cast_column("audio", Audio(decode=False))

    with args.manifest.open("w", encoding="utf-8") as manifest_file:
        written = 0
        for index, example in enumerate(dataset):
            if index >= args.n_samples:
                break

            audio = example["audio"]
            text = str(example["text"]).strip()
            extension = Path(audio.get("path") or "").suffix or ".flac"
            filename = f"{index:04d}-{sample_id(example, index)}{extension}"
            audio_path = args.output_dir / filename

            if not audio_path.exists():
                write_audio_bytes(audio_path, audio["bytes"])

            entry = {
                "audio_filepath": str(audio_path.relative_to(args.manifest.parent)),
                "text": text,
            }
            manifest_file.write(json.dumps(entry, ensure_ascii=False) + "\n")
            manifest_file.flush()
            written += 1

            print(f"[{index + 1:>3}/{args.n_samples}] {filename}")

    print()
    print(f"Wrote {written} samples to {args.output_dir}")
    print(f"Wrote manifest to {args.manifest}")


if __name__ == "__main__":
    main()
