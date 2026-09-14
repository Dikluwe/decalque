#!/usr/bin/env python3
"""Emit real PaddleOCR text-line polygons without estimated word boxes."""

from __future__ import annotations

import argparse
import contextlib
import json
import sys
from typing import Any


def normalize_result(raw: dict[str, Any]) -> dict[str, Any]:
    result = raw["res"]
    fields = [
        result.get("rec_texts", []),
        result.get("rec_scores", []),
        result.get("rec_polys", []),
        result.get("rec_boxes", []),
    ]
    if len({len(field) for field in fields}) != 1:
        raise ValueError("PaddleOCR retornou listas de linhas com tamanhos diferentes")
    lines = []
    for index, (text, confidence, polygon, bbox) in enumerate(
        zip(*fields)
    ):
        lines.append(
            {
                "id": index,
                "text": text,
                "recognition_confidence": confidence,
                "polygon": polygon,
                "bbox": bbox,
            }
        )
    return {"page_index": result.get("page_index"), "lines": lines}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("input")
    parser.add_argument("--lang", default="en")
    parser.add_argument("--ocr-version", default="PP-OCRv5")
    parser.add_argument("--device", default="cpu")
    return parser.parse_args()


def run_provider(input_path: str, lang: str, ocr_version: str, device: str) -> list[dict[str, Any]]:
    from paddleocr import PaddleOCR

    pipeline = PaddleOCR(
        lang=lang,
        ocr_version=ocr_version,
        device=device,
        enable_mkldnn=False,
        use_doc_orientation_classify=False,
        use_doc_unwarping=False,
        use_textline_orientation=False,
        return_word_box=False,
    )
    results = pipeline.predict(input_path, return_word_box=False)
    return [normalize_result(result.json) for result in results]


def main() -> int:
    args = parse_args()
    try:
        with contextlib.redirect_stdout(sys.stderr):
            pages = run_provider(args.input, args.lang, args.ocr_version, args.device)
        json.dump(
            {"schema_version": 1, "provider": "paddleocr", "pages": pages},
            sys.stdout,
            ensure_ascii=False,
            separators=(",", ":"),
        )
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"paddle-line-detector: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
