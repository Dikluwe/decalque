#!/usr/bin/env python3
"""Cluster captured OCR regions, recognizing mirrored recurring symbols."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

from PIL import Image, ImageChops, ImageOps


SIZE = 64


def normalized_ink(path: Path) -> Image.Image:
    with Image.open(path) as source:
        gray = ImageOps.grayscale(source)
        mask = gray.point(lambda value: 255 if value < 210 else 0, mode="1").convert("L")
    bounds = mask.getbbox()
    if not bounds:
        return Image.new("L", (SIZE, SIZE), 0)
    ink = mask.crop(bounds)
    scale = min(SIZE / ink.width, SIZE / ink.height)
    ink = ink.resize(
        (max(1, round(ink.width * scale)), max(1, round(ink.height * scale))),
        Image.Resampling.LANCZOS,
    )
    canvas = Image.new("L", (SIZE, SIZE), 0)
    canvas.paste(ink, ((SIZE - ink.width) // 2, (SIZE - ink.height) // 2))
    return canvas.point(lambda value: 255 if value >= 128 else 0)


def similarity(left: Image.Image, right: Image.Image) -> float:
    union = ImageChops.lighter(left, right).histogram()[255]
    if union == 0:
        return 0.0
    intersection = ImageChops.darker(left, right).histogram()[255]
    return intersection / union


def best_similarity(left: Image.Image, right: Image.Image) -> tuple[float, str]:
    direct = similarity(left, right)
    mirrored = similarity(left, ImageOps.mirror(right))
    return (direct, "original") if direct >= mirrored else (mirrored, "mirrored")


def build_catalog(reports: list[Path], threshold: float) -> dict[str, Any]:
    clusters: list[dict[str, Any]] = []
    occurrences = []
    for report_path in reports:
        report = json.loads(report_path.read_text())
        for asset in report.get("assets", []):
            path = Path(asset["path"])
            signature = normalized_ink(path)
            matches = [best_similarity(cluster["signature"], signature) for cluster in clusters]
            if matches and max(score for score, _ in matches) >= threshold:
                index = max(range(len(matches)), key=lambda item: matches[item][0])
                score, orientation = matches[index]
            else:
                index = len(clusters)
                score, orientation = 1.0, "original"
                clusters.append({"signature": signature, "representative": str(path.resolve())})
            occurrences.append({
                "symbol_id": f"symbol-{index + 1:03d}",
                "report": str(report_path.resolve()),
                "asset": str(path.resolve()),
                "similarity": round(score, 4),
                "orientation": orientation,
            })
    return {
        "schema_version": 1,
        "threshold": threshold,
        "symbols": [{
            "symbol_id": f"symbol-{index + 1:03d}",
            "representative": cluster["representative"],
            "occurrences": [item for item in occurrences if item["symbol_id"] == f"symbol-{index + 1:03d}"],
        } for index, cluster in enumerate(clusters)],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("reports", nargs="+", type=Path)
    parser.add_argument("--threshold", type=float, default=0.72)
    args = parser.parse_args()
    try:
        json.dump(build_catalog(args.reports, args.threshold), sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"visual-symbol-catalog: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
