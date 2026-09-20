#!/usr/bin/env python3
"""Derive word boxes from observed ink gaps inside detected text lines."""

from __future__ import annotations

import argparse
from collections import Counter
import json
import math
import re
import sys
from pathlib import Path
from typing import Any

import typographic_profile


WORDS = re.compile(r"\S+", re.UNICODE)


def margin_word_anchors(
    ink: list[list[bool]],
    text: str,
    line_bbox: list[int],
    minimum_word_gap: int,
    edge_band_ratio: float = 0.5,
) -> dict[str, Any]:
    """Observe only the first and last word using gaps near the line edges."""
    unknown = {"margin_anchor_status": "unknown"}
    words = WORDS.findall(text or "")
    if (not words or not ink or not ink[0] or len(line_bbox) != 4 or
            minimum_word_gap < 1 or not 0 < edge_band_ratio <= 0.5):
        return unknown
    width = len(ink[0])
    active = [any(row[x] for row in ink) for x in range(width)]
    components = []
    x = 0
    while x < width:
        if not active[x]:
            x += 1
            continue
        start = x
        while x + 1 < width and active[x + 1]:
            x += 1
        components.append((start, x + 1))
        x += 1
    if not components:
        return unknown

    def observed_box(start: int, end: int) -> list[int]:
        ys = [y for y, row in enumerate(ink) if any(row[start:end])]
        return [line_bbox[0] + start, line_bbox[1] + min(ys),
                line_bbox[0] + end, line_bbox[1] + max(ys) + 1]

    if len(words) == 1:
        box = observed_box(components[0][0], components[-1][1])
        anchor = {"text": words[0], "bbox": box, "boundary_gap_px": None,
                  "source": "margin-ink-gap"}
        return {"margin_anchor_status": "observed", "left": anchor,
                "right": dict(anchor),
                "anchored_text_bbox": [box[0], line_bbox[1], box[2], line_bbox[3]]}

    gaps = [(components[index][1], components[index + 1][0],
             components[index + 1][0] - components[index][1], index)
            for index in range(len(components) - 1)]
    left_candidates = [gap for gap in gaps
                       if gap[2] >= minimum_word_gap and gap[0] <= width * edge_band_ratio]
    right_candidates = [gap for gap in gaps
                        if gap[2] >= minimum_word_gap
                        and gap[1] >= width * (1 - edge_band_ratio) - 1]
    if not left_candidates or not right_candidates:
        return unknown
    left_gap = min(left_candidates, key=lambda gap: gap[0])
    right_gap = max(right_candidates, key=lambda gap: gap[1])
    left_box = observed_box(components[0][0], components[left_gap[3]][1])
    right_box = observed_box(components[right_gap[3] + 1][0], components[-1][1])
    return {
        "margin_anchor_status": "observed",
        "left": {"text": words[0], "bbox": left_box, "boundary_gap_px": left_gap[2],
                 "source": "margin-ink-gap"},
        "right": {"text": words[-1], "bbox": right_box, "boundary_gap_px": right_gap[2],
                  "source": "margin-ink-gap"},
        "anchored_text_bbox": [left_box[0], line_bbox[1], right_box[2], line_bbox[3]],
    }


def estimate_baseline(
    ink: list[list[bool]], start_x: int, end_x: int, offset_y: int = 0
) -> tuple[int | None, float]:
    """Estimate the baseline boundary from dominant per-column ink bottoms."""
    if not ink or not ink[0] or start_x < 0 or end_x > len(ink[0]) or start_x >= end_x:
        return None, 0.0
    bottoms = [
        max(y for y, row in enumerate(ink) if row[x])
        for x in range(start_x, end_x)
        if any(row[x] for row in ink)
    ]
    if not bottoms:
        return None, 0.0
    candidates = Counter(bottoms)
    baseline = min(
        candidates,
        key=lambda candidate: (
            -sum(abs(bottom - candidate) <= 1 for bottom in bottoms),
            candidate,
        ),
    )
    support = sum(abs(bottom - baseline) <= 1 for bottom in bottoms)
    return offset_y + baseline + 1, support / len(bottoms)


def ink_segments(
    ink: list[list[bool]], offset_x: int, offset_y: int, minimum_word_gap: int
) -> list[list[int]]:
    if not ink or not ink[0] or minimum_word_gap < 1:
        return []
    width = len(ink[0])
    active = [any(row[x] for row in ink) for x in range(width)]
    components = []
    x = 0
    while x < width:
        if not active[x]:
            x += 1
            continue
        start = x
        while x + 1 < width and active[x + 1]:
            x += 1
        components.append([start, x])
        x += 1
    if not components:
        return []

    groups = [components[0][:]]
    for start, end in components[1:]:
        gap = start - groups[-1][1] - 1
        if gap < minimum_word_gap:
            groups[-1][1] = end
        else:
            groups.append([start, end])

    boxes = []
    for start, end in groups:
        ys = [y for y, row in enumerate(ink) if any(row[start : end + 1])]
        boxes.append(
            [offset_x + start, offset_y + min(ys), offset_x + end + 1, offset_y + max(ys) + 1]
        )
    return boxes


def build_word_segments(
    line: dict[str, Any], boxes: list[list[int]], minimum_gap: int
) -> None:
    line["word_segments"] = []
    line["word_geometry_status"] = "unknown"
    words = WORDS.findall(line.get("text") or "")
    if not words or len(words) != len(boxes):
        return
    line["word_geometry_status"] = "observed"
    line["word_segments"] = [
        {
            "id": f'{line["id"]}:{index}',
            "text": word,
            "bbox": box,
            "polygon": [
                [box[0], box[1]],
                [box[2], box[1]],
                [box[2], box[3]],
                [box[0], box[3]],
            ],
            "association_confidence": 1.0,
            "source": "ink-gap-segmentation",
            "minimum_word_gap_px": minimum_gap,
        }
        for index, (word, box) in enumerate(zip(words, boxes))
    ]


def attach_word_geometry(
    page: dict[str, Any], image: Any, minimum_gap_ratio: float = 0.18
) -> dict[str, Any]:
    import cv2

    height, width = image.shape[:2]
    for region in page.get("regions", []):
        for line in region.get("detected_lines", []):
            build_word_segments(line, [], 0)
            bbox = line.get("bbox")
            if not isinstance(bbox, list) or len(bbox) != 4:
                continue
            x0, y0, x1, y1 = [int(round(value)) for value in bbox]
            x0, y0 = max(0, x0), max(0, y0)
            x1, y1 = min(width, x1), min(height, y1)
            if x0 >= x1 or y0 >= y1:
                continue
            crop = image[y0:y1, x0:x1]
            gray = cv2.cvtColor(crop, cv2.COLOR_BGR2GRAY) if crop.ndim == 3 else crop
            _, binary = cv2.threshold(gray, 0, 255, cv2.THRESH_BINARY_INV + cv2.THRESH_OTSU)
            ink = binary > 0
            minimum_gap = max(2, int(math.ceil((y1 - y0) * minimum_gap_ratio)))
            boxes = ink_segments(ink.tolist(), x0, y0, minimum_gap)
            build_word_segments(line, boxes, minimum_gap)
            ink_rows = ink.tolist()
            line.update(margin_word_anchors(
                ink_rows, line.get("text") or "", [x0, y0, x1, y1], minimum_gap
            ))
            for segment in line["word_segments"]:
                baseline, confidence = estimate_baseline(
                    ink_rows,
                    segment["bbox"][0] - x0,
                    segment["bbox"][2] - x0,
                    y0,
                )
                segment["baseline_y_px"] = baseline
                segment["baseline_confidence"] = confidence
                segment["baseline_source"] = "dominant-column-ink-bottom"
                local_x0 = segment["bbox"][0] - x0
                local_y0 = segment["bbox"][1] - y0
                local_x1 = segment["bbox"][2] - x0
                local_y1 = segment["bbox"][3] - y0
                segment["ink_shape"] = typographic_profile.ink_shape_descriptor(
                    [row[local_x0:local_x1] for row in ink_rows[local_y0:local_y1]]
                )

        segments = [
            segment
            for line in region.get("detected_lines", [])
            for segment in line.get("word_segments", [])
        ]
        for semantic_line in region.get("lines", []):
            for token in semantic_line.get("tokens", []):
                if token.get("kind") == "whitespace":
                    continue
                matches = [
                    segment
                    for segment in segments
                    if segment["text"].casefold() == (token.get("text") or "").casefold()
                    and segment["id"].startswith(f'{token.get("line_geometry_ref")}:')
                ]
                if len(matches) == 1:
                    token["bbox"] = matches[0]["bbox"]
                    token["polygon"] = matches[0]["polygon"]
                    token["geometry_source"] = matches[0]["source"]
    return page


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("scan_json", type=Path)
    parser.add_argument("image", type=Path)
    parser.add_argument("--minimum-gap-ratio", type=float, default=0.18)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        import cv2

        pages = json.loads(args.scan_json.read_text(encoding="utf-8"))
        if len(pages) != 1:
            raise ValueError("esta versão aceita uma imagem de uma página")
        image = cv2.imread(str(args.image), cv2.IMREAD_COLOR)
        if image is None:
            raise ValueError("imagem não pôde ser lida")
        if pages[0].get("width") != image.shape[1] or pages[0].get("height") != image.shape[0]:
            raise ValueError("dimensões da imagem diferem da observação VLM")
        output = [attach_word_geometry(pages[0], image, args.minimum_gap_ratio)]
        json.dump(output, sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"word-geometry-detector: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
