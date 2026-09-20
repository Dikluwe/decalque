#!/usr/bin/env python3
"""Find a known symbol at any scale in page pixels and capture its occurrences."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image
from scipy import ndimage

from visual_symbol_catalog import best_similarity, normalized_ink


def find_symbol(template_path: Path, pages: list[Path], asset_dir: Path, threshold: float) -> dict:
    template = normalized_ink(template_path)
    detections = []
    asset_dir.mkdir(parents=True, exist_ok=True)
    for page_index, page_path in enumerate(pages):
        with Image.open(page_path) as source:
            gray = source.convert("L")
            labels, _ = ndimage.label(np.asarray(gray) < 210)
            candidates = []
            for slices in ndimage.find_objects(labels):
                if slices is None:
                    continue
                vertical, horizontal = slices
                bbox = [horizontal.start, vertical.start, horizontal.stop, vertical.stop]
                width, height = bbox[2] - bbox[0], bbox[3] - bbox[1]
                if width < 20 or height < 10 or width > gray.width // 3 or height > gray.height // 5:
                    continue
                candidate_path = asset_dir / ".candidate.png"
                gray.crop(tuple(bbox)).save(candidate_path)
                score, orientation = best_similarity(template, normalized_ink(candidate_path))
                if score >= threshold:
                    candidates.append((score, orientation, bbox))
            if not candidates:
                continue
            score, orientation, bbox = max(candidates, key=lambda item: item[0])
            margin = max(4, round(max(bbox[2] - bbox[0], bbox[3] - bbox[1]) * 0.12))
            crop_bbox = [max(0, bbox[0] - margin), max(0, bbox[1] - margin),
                         min(gray.width, bbox[2] + margin), min(gray.height, bbox[3] + margin)]
            output = asset_dir / f"page-{page_index + 1:03d}.png"
            gray.crop(tuple(crop_bbox)).save(output)
            detections.append({
                "page_input": str(page_path.resolve()), "bbox": bbox, "crop_bbox": crop_bbox,
                "similarity": round(score, 4), "orientation": orientation,
                "path": str(output.resolve()),
            })
    candidate_path = asset_dir / ".candidate.png"
    if candidate_path.exists():
        candidate_path.unlink()
    return {"schema_version": 1, "template": str(template_path.resolve()),
            "threshold": threshold, "detections": detections}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("template", type=Path)
    parser.add_argument("pages", nargs="+", type=Path)
    parser.add_argument("--asset-dir", type=Path, required=True)
    parser.add_argument("--threshold", type=float, default=0.65)
    args = parser.parse_args()
    try:
        json.dump(find_symbol(args.template, args.pages, args.asset_dir, args.threshold),
                  sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"recurring-symbol-finder: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
