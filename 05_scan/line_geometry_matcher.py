#!/usr/bin/env python3
"""Attach observed PaddleOCR line geometry to VLM scan regions and tokens."""

from __future__ import annotations

import argparse
import copy
import difflib
import json
import re
import sys
from pathlib import Path
from typing import Any


NON_WORD = re.compile(r"\W+", re.UNICODE)


def normalized(text: str | None) -> str:
    return NON_WORD.sub("", (text or "").casefold())


def compatibility(line_text: str | None, region_text: str | None) -> float:
    line = normalized(line_text)
    region = normalized(region_text)
    if not line or not region:
        return 0.0
    if line in region:
        return 1.0
    return difflib.SequenceMatcher(None, line, region, autojunk=False).ratio()


def center(bbox: list[float]) -> tuple[float, float]:
    return (bbox[0] + bbox[2]) / 2, (bbox[1] + bbox[3]) / 2


def contains(region_bbox: list[float] | None, point: tuple[float, float]) -> bool:
    return bool(
        region_bbox
        and region_bbox[0] <= point[0] <= region_bbox[2]
        and region_bbox[1] <= point[1] <= region_bbox[3]
    )


def attach_token_references(region: dict[str, Any]) -> None:
    detected = region.get("detected_lines", [])
    for line in region.get("lines", []):
        for token in line.get("tokens", []):
            token["line_geometry_ref"] = None
            needle = normalized(token.get("text"))
            if not needle or token.get("kind") == "whitespace":
                continue
            matches = [item["id"] for item in detected if needle in normalized(item["text"])]
            if len(matches) == 1:
                token["line_geometry_ref"] = matches[0]


def enrich_page(
    page: dict[str, Any], detected_page: dict[str, Any], minimum_compatibility: float = 0.5
) -> dict[str, Any]:
    output = copy.deepcopy(page)
    for region in output.get("regions", []):
        region["detected_lines"] = []
    unassigned = []
    for observed in detected_page.get("lines", []):
        point = center(observed["bbox"])
        candidates = []
        for region_index, region in enumerate(output.get("regions", [])):
            score = compatibility(observed.get("text"), region.get("text"))
            if contains(region.get("bbox"), point) and score >= minimum_compatibility:
                candidates.append((score, region_index))
        if not candidates:
            unassigned.append(copy.deepcopy(observed))
            continue
        candidates.sort(reverse=True)
        if len(candidates) > 1 and candidates[0][0] == candidates[1][0]:
            unassigned.append(copy.deepcopy(observed))
            continue
        score, region_index = candidates[0]
        attached = copy.deepcopy(observed)
        attached["association_confidence"] = score
        attached["source"] = "paddleocr-line-detection"
        output["regions"][region_index]["detected_lines"].append(attached)

    for region in output.get("regions", []):
        region["detected_lines"].sort(key=lambda item: (item["bbox"][1], item["bbox"][0]))
        attach_token_references(region)
    output["unassigned_lines"] = unassigned
    return output


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("scan_json", type=Path)
    parser.add_argument("line_json", type=Path)
    parser.add_argument("--minimum-compatibility", type=float, default=0.5)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        pages = json.loads(args.scan_json.read_text(encoding="utf-8"))
        detected = json.loads(args.line_json.read_text(encoding="utf-8"))["pages"]
        if len(pages) != len(detected):
            raise ValueError("quantidade de páginas difere entre VLM e detector de linhas")
        output = [
            enrich_page(page, detected_page, args.minimum_compatibility)
            for page, detected_page in zip(pages, detected)
        ]
        json.dump(output, sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"line-geometry-matcher: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
