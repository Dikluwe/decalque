#!/usr/bin/env python3
"""PaddleOCR-VL layout adapter using LM Studio as the VLM backend."""

from __future__ import annotations

import argparse
import contextlib
import json
import os
import re
import sys
from typing import Any


TOKEN_PATTERN = re.compile(r"\w+|\s+|[^\w\s]+", re.UNICODE)


def unknown_font() -> dict[str, Any]:
    """Return an explicit lack of typographic evidence."""
    return {
        "status": "unknown",
        "family": None,
        "style": None,
        "weight": None,
        "size_px": None,
        "confidence": None,
        "evidence": [],
    }


def tokenize_line(text: str) -> list[dict[str, Any]]:
    tokens = []
    for match in TOKEN_PATTERN.finditer(text):
        value = match.group(0)
        if value.isspace():
            kind = "whitespace"
        elif value[0].isalnum() or value[0] == "_":
            kind = "word"
        else:
            kind = "punctuation"
        tokens.append(
            {
                "kind": kind,
                "text": value,
                "span": [match.start(), match.end()],
                "bbox": None,
                "polygon": None,
                "recognition_confidence": None,
                "font": unknown_font(),
            }
        )
    return tokens


def derive_lines(text: str | None) -> list[dict[str, Any]]:
    if not text:
        return []
    lines = []
    offset = 0
    for part in text.splitlines(keepends=True):
        if part.endswith("\r\n"):
            content, break_after = part[:-2], "crlf"
        elif part.endswith("\n"):
            content, break_after = part[:-1], "lf"
        elif part.endswith("\r"):
            content, break_after = part[:-1], "cr"
        else:
            content, break_after = part, None
        lines.append(
            {
                "text": content,
                "span": [offset, offset + len(content)],
                "break_after": break_after,
                "bbox": None,
                "polygon": None,
                "recognition_confidence": None,
                "tokens": tokenize_line(content),
            }
        )
        offset += len(part)
    return lines


def normalize_result(raw: dict[str, Any], model: str) -> dict[str, Any]:
    result = raw["res"]
    detections = result.get("layout_det_res", {}).get("boxes", [])
    confidence_by_order = {
        item.get("order"): item.get("score")
        for item in detections
        if item.get("order") is not None
    }
    regions = []
    for source_index, block in enumerate(result.get("parsing_res_list", [])):
        order = block.get("block_order")
        text = block.get("block_content")
        regions.append(
            {
                "label": block.get("block_label"),
                "text": text,
                "bbox": block.get("block_bbox"),
                "polygon": block.get("block_polygon_points"),
                "layout_confidence": confidence_by_order.get(order),
                "recognition_confidence": None,
                "order": order,
                "lines": derive_lines(text),
                "_source_index": source_index,
            }
        )
    regions.sort(
        key=lambda region: (
            region["order"] is None,
            region["order"] if region["order"] is not None else 0,
            region["_source_index"],
        )
    )
    for region in regions:
        del region["_source_index"]
    return {
        "schema_version": 2,
        "provider": "paddleocr-vl+lm-studio",
        "model": model,
        "coordinate_space": "image-pixels-ydown",
        "width": result["width"],
        "height": result["height"],
        "page_index": result.get("page_index"),
        "regions": regions,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("input")
    parser.add_argument(
        "--base-url",
        default=os.environ.get("LM_STUDIO_BASE_URL", "http://127.0.0.1:1234/v1"),
    )
    parser.add_argument("--model", default="paddleocr-vl")
    parser.add_argument("--device", default="cpu")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        from paddleocr import PaddleOCRVL

        with contextlib.redirect_stdout(sys.stderr):
            pipeline = PaddleOCRVL(
                pipeline_version="v1.6",
                vl_rec_backend="llama-cpp-server",
                vl_rec_server_url=args.base_url,
                vl_rec_api_model_name=args.model,
                device=args.device,
                use_doc_orientation_classify=False,
                use_doc_unwarping=False,
                use_queues=False,
            )
            results = pipeline.predict(args.input, use_queues=False)
            normalized = [normalize_result(result.json, args.model) for result in results]
        json.dump(normalized, sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:  # provider boundary: preserve failure as process error
        print(f"paddle-lmstudio: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
