#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/scan-observation-exporter.md
"""Run Paddle line OCR and atomically emit Decalque ScanObservation v1."""

from __future__ import annotations

import argparse
import contextlib
import sys
from importlib.metadata import version
from pathlib import Path

from paddle_line_detector import run_provider
from scan_observation_v1 import build_observation, write_observation_atomic


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("raster", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--page-index", type=int, required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--lang", default="en")
    parser.add_argument("--ocr-version", default="PP-OCRv5")
    parser.add_argument("--device", default="cpu")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        with contextlib.redirect_stdout(sys.stderr):
            pages = run_provider(
                str(args.raster), args.lang, args.ocr_version, args.device
            )
        if len(pages) != 1:
            raise ValueError("Paddle deve retornar exatamente uma pagina")
        observation = build_observation(
            args.raster,
            pages[0],
            page_index=args.page_index,
            run_id=args.run_id,
            lang=args.lang,
            ocr_version=args.ocr_version,
            device=args.device,
            paddle_version=version("paddleocr"),
        )
        write_observation_atomic(args.output, observation)
        return 0
    except Exception as error:
        print(f"paddle-scan-observation: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
