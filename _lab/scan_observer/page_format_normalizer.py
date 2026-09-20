#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/page-format-normalization.md
"""Snap a scanned page canvas to a known physical format without resampling ink."""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from PIL import Image


PT_PER_MM = 72.0 / 25.4


@dataclass(frozen=True)
class PageFormat:
    name: str
    width_mm: float
    height_mm: float
    category: str


FORMATS = (
    PageFormat("ISO-A3", 297, 420, "iso"),
    PageFormat("ISO-A4", 210, 297, "iso"),
    PageFormat("ISO-A5", 148, 210, "iso"),
    PageFormat("ISO-A6", 105, 148, "iso"),
    PageFormat("ISO-B5", 176, 250, "iso"),
    PageFormat("JIS-B5", 182, 257, "jis"),
    PageFormat("US-Letter", 215.9, 279.4, "north-american"),
    PageFormat("US-Legal", 215.9, 355.6, "north-american"),
    PageFormat("US-Executive", 184.15, 266.7, "north-american"),
    PageFormat("Book-130x200", 130, 200, "editorial"),
    PageFormat("Book-145x215", 145, 215, "editorial"),
    PageFormat("Book-150x230", 150, 230, "editorial"),
    PageFormat("Book-160x220", 160, 220, "editorial"),
    PageFormat("Book-170x240", 170, 240, "editorial"),
    PageFormat("Book-6x9", 152.4, 228.6, "editorial"),
)


def _candidates() -> list[tuple[PageFormat, str, float, float]]:
    values = []
    for item in FORMATS:
        values.append((item, "portrait", item.width_mm, item.height_mm))
        values.append((item, "landscape", item.height_mm, item.width_mm))
    return values


def identify(width_pt: float, height_pt: float, tolerance: float = 0.03) -> dict[str, Any]:
    if min(width_pt, height_pt) <= 0:
        raise ValueError("dimensoes fisicas devem ser positivas")
    if not 0 < tolerance <= 0.20:
        raise ValueError("tolerance deve estar entre 0 e 0.20")
    width_mm, height_mm = width_pt / PT_PER_MM, height_pt / PT_PER_MM
    ranked = []
    for item, orientation, target_width, target_height in _candidates():
        width_error = abs(width_mm - target_width) / target_width
        height_error = abs(height_mm - target_height) / target_height
        ranked.append((max(width_error, height_error), width_error + height_error,
                       item, orientation, target_width, target_height))
    error, _, item, orientation, target_width, target_height = min(
        ranked, key=lambda value: (value[0], value[1], value[2].name, value[3])
    )
    base = {"observed_mm": [round(width_mm, 4), round(height_mm, 4)],
            "maximum_relative_error": round(error, 6), "tolerance": tolerance}
    if error > tolerance:
        return {"status": "unknown", "reason": "no-format-within-tolerance", **base}
    return {"status": "matched", "format": item.name, "category": item.category,
            "orientation": orientation, "target_mm": [target_width, target_height],
            "confidence": round(max(0.0, 1.0 - error / tolerance), 6), **base}


def _axis_plan(source: int, target: int, alignment: str) -> tuple[int, int, int]:
    delta = target - source
    if alignment == "start":
        before = 0
    elif alignment == "end":
        before = delta if delta > 0 else delta
    else:
        before = delta // 2
    source_start = max(0, -before)
    destination_start = max(0, before)
    extent = min(source - source_start, target - destination_start)
    return source_start, destination_start, extent


def normalize(source: Image.Image, width_pt: float, height_pt: float,
              side: str = "unknown", tolerance: float = 0.03) -> tuple[Image.Image, dict[str, Any]]:
    match = identify(width_pt, height_pt, tolerance)
    if match["status"] != "matched":
        return source.copy(), {**match, "source_px": list(source.size),
                               "target_px": list(source.size), "translation_px": [0, 0],
                               "resampled": False}
    width_mm, height_mm = match["observed_mm"]
    dpi_x = source.width / (width_mm / 25.4)
    dpi_y = source.height / (height_mm / 25.4)
    dpi = statistics.median((dpi_x, dpi_y))
    dpi_disagreement = abs(dpi_x - dpi_y) / dpi
    if dpi_disagreement > 0.02:
        return source.copy(), {**match, "status": "unknown", "reason": "inconsistent-dpi",
                               "dpi_xy": [round(dpi_x, 4), round(dpi_y, 4)],
                               "source_px": list(source.size), "target_px": list(source.size),
                               "translation_px": [0, 0], "resampled": False}
    target_width = round(match["target_mm"][0] / 25.4 * dpi)
    target_height = round(match["target_mm"][1] / 25.4 * dpi)
    horizontal = "start" if side == "right" else "end" if side == "left" else "center"
    sx, dx, width = _axis_plan(source.width, target_width, horizontal)
    sy, dy, height = _axis_plan(source.height, target_height, "center")
    canvas = Image.new(source.mode, (target_width, target_height), "white")
    canvas.paste(source.crop((sx, sy, sx + width, sy + height)), (dx, dy))
    report = {**match, "status": "normalized", "side": side,
              "source_pt": [width_pt, height_pt], "target_pt": [
                  round(match["target_mm"][0] * PT_PER_MM, 4),
                  round(match["target_mm"][1] * PT_PER_MM, 4)],
              "source_px": list(source.size), "target_px": [target_width, target_height],
              "dpi_xy": [round(dpi_x, 4), round(dpi_y, 4)], "effective_dpi": round(dpi, 4),
              "crop_box_px": [sx, sy, sx + width, sy + height],
              "paste_origin_px": [dx, dy], "translation_px": [dx - sx, dy - sy],
              "resampled": False}
    return canvas, report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--width-pt", type=float, required=True)
    parser.add_argument("--height-pt", type=float, required=True)
    parser.add_argument("--side", choices=("left", "right", "unknown"), default="unknown")
    parser.add_argument("--tolerance", type=float, default=0.03)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    try:
        with Image.open(args.source) as image:
            result, report = normalize(image.convert("RGB"), args.width_pt, args.height_pt,
                                       args.side, args.tolerance)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        result.save(args.output)
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
        return 0
    except Exception as error:
        print(f"page-format-normalizer: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
