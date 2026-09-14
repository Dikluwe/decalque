#!/usr/bin/env python3
"""Derive conservative typographic measurements from observed word ink."""

from __future__ import annotations

import argparse
import copy
import json
import math
import sys
import unicodedata
from pathlib import Path
from typing import Any


X_HEIGHT_LETTERS = frozenset("acemnorsuvwxz")
ASCENDER_LETTERS = frozenset("bdfhklt")
DESCENDER_LETTERS = frozenset("gjpqy")


def ink_shape_descriptor(
    ink: list[list[bool]], bins: int = 8
) -> dict[str, Any] | None:
    if not ink or not ink[0] or bins < 1:
        return None
    height, width = len(ink), len(ink[0])
    points = [(x, y) for y, row in enumerate(ink) for x, value in enumerate(row) if value]
    if not points:
        return None

    def projection(axis: int, extent: int, other_extent: int) -> list[float]:
        values = []
        for index in range(bins):
            start = index * extent // bins
            end = (index + 1) * extent // bins
            area = max(1, (end - start) * other_extent)
            count = sum(start <= point[axis] < end for point in points)
            values.append(count / area)
        return values

    occupancy = []
    for row in range(8):
        y0, y1 = row * height // 8, (row + 1) * height // 8
        for column in range(16):
            x0, x1 = column * width // 16, (column + 1) * width // 16
            area = max(1, (y1 - y0) * (x1 - x0))
            count = sum(x0 <= x < x1 and y0 <= y < y1 for x, y in points)
            occupancy.append(count / area)

    return {
        "version": 1,
        "bins": bins,
        "density": len(points) / (width * height),
        "aspect_ratio": width / height,
        "centroid_x": sum(x + 0.5 for x, _ in points) / len(points) / width,
        "centroid_y": sum(y + 0.5 for _, y in points) / len(points) / height,
        "horizontal_projection": projection(1, height, width),
        "vertical_projection": projection(0, width, height),
        "occupancy_rows": 8,
        "occupancy_columns": 16,
        "occupancy": occupancy,
    }


def finite_number(value: Any) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
    )


def letter_features(text: str) -> dict[str, bool]:
    decomposed = unicodedata.normalize("NFD", text)
    bases = [character.casefold() for character in decomposed if character.isalpha()]
    marks = [character for character in decomposed if unicodedata.combining(character)]
    has_uppercase = any(character.isupper() for character in text if character.isalpha())
    return {
        "x_height_only": bool(bases)
        and not marks
        and not has_uppercase
        and all(c in X_HEIGHT_LETTERS for c in bases),
        "has_ascender": has_uppercase or any(c in ASCENDER_LETTERS for c in bases),
        "has_descender": any(c in DESCENDER_LETTERS for c in bases)
        or "\N{COMBINING CEDILLA}" in marks,
        "has_marks": bool(marks),
    }


def profile_for_segment(
    segment: dict[str, Any], minimum_baseline_confidence: float = 0.5
) -> dict[str, Any]:
    bbox_px = segment.get("bbox")
    bbox_pt = segment.get("bbox_pt")
    baseline_px = segment.get("baseline_y_px")
    baseline_pt = segment.get("baseline_y_pt")
    confidence = segment.get("baseline_confidence")
    if (
        not isinstance(bbox_px, list)
        or len(bbox_px) != 4
        or not isinstance(bbox_pt, list)
        or len(bbox_pt) != 4
        or not all(finite_number(value) for value in bbox_px + bbox_pt)
        or not finite_number(baseline_px)
        or not finite_number(baseline_pt)
        or not finite_number(confidence)
        or confidence < minimum_baseline_confidence
        or not (bbox_px[1] <= baseline_px <= bbox_px[3])
        or not (bbox_pt[1] <= baseline_pt <= bbox_pt[3])
    ):
        return {
            "status": "unknown",
            "reason": "insufficient-baseline-or-geometry",
            "confidence": confidence if finite_number(confidence) else None,
        }

    features = letter_features(segment.get("text") or "")
    top_px, bottom_px = bbox_px[1], bbox_px[3]
    top_pt, bottom_pt = bbox_pt[1], bbox_pt[3]
    result = {
        "status": "observed",
        "source": "word-ink-envelope",
        "confidence": confidence,
        "features": features,
        "ink_shape": segment.get("ink_shape"),
        "ink_height_px": bottom_px - top_px,
        "ink_height_pt": bottom_pt - top_pt,
        "x_height_px": None,
        "x_height_pt": None,
        "ascender_height_px": None,
        "ascender_height_pt": None,
        "descender_depth_px": None,
        "descender_depth_pt": None,
    }
    if features["x_height_only"]:
        result["x_height_px"] = baseline_px - top_px
        result["x_height_pt"] = baseline_pt - top_pt
    if features["has_ascender"]:
        result["ascender_height_px"] = baseline_px - top_px
        result["ascender_height_pt"] = baseline_pt - top_pt
    if features["has_descender"]:
        result["descender_depth_px"] = bottom_px - baseline_px
        result["descender_depth_pt"] = bottom_pt - baseline_pt
    return result


def enrich_page(
    page: dict[str, Any], minimum_baseline_confidence: float = 0.5
) -> dict[str, Any]:
    output = copy.deepcopy(page)
    for region in output.get("regions", []):
        for line in region.get("detected_lines", []):
            for segment in line.get("word_segments", []):
                segment["typographic_profile"] = profile_for_segment(
                    segment, minimum_baseline_confidence
                )
    return output


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("scan_json", type=Path)
    parser.add_argument("--minimum-baseline-confidence", type=float, default=0.5)
    args = parser.parse_args()
    try:
        pages = json.loads(args.scan_json.read_text(encoding="utf-8"))
        output = [enrich_page(page, args.minimum_baseline_confidence) for page in pages]
        json.dump(output, sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"typographic-profile: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
