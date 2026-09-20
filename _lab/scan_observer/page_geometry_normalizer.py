#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/frozen-document-typography.md
"""Normalize page skew while preserving an explicit coordinate transform."""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image


def _projection_score(image: Image.Image, angle: float) -> float:
    rotated = image.rotate(angle, resample=Image.Resampling.BILINEAR, fillcolor=255)
    array = np.asarray(rotated, dtype=np.uint8)
    ink = array < 190
    projection = ink.sum(axis=1).astype(float)
    return float(np.var(projection))


def _inverse_affine(width: int, height: int, applied_deg: float) -> list[float]:
    angle = math.radians(-applied_deg)
    cosine, sine = math.cos(angle), math.sin(angle)
    cx, cy = width / 2, height / 2
    return [round(cosine, 10), round(-sine, 10), round(cx - cosine * cx + sine * cy, 6),
            round(sine, 10), round(cosine, 10), round(cy - sine * cx - cosine * cy, 6)]


def normalize_image(source: Image.Image, max_angle: float = 3.0,
                    step: float = 0.1) -> tuple[Image.Image, dict[str, Any]]:
    gray = source.convert("L")
    preview = gray.copy()
    preview.thumbnail((600, 900), Image.Resampling.LANCZOS)
    ink_ratio = float((np.asarray(preview) < 190).mean())
    if ink_ratio < 0.001:
        return source.copy(), {"status": "unknown", "reason": "insufficient-ink",
                               "observed_skew_deg": None, "applied_rotation_deg": 0.0,
                               "perspective_status": "unknown",
                               "inverse_affine": [1, 0, 0, 0, 1, 0]}
    angles = np.arange(-max_angle, max_angle + step / 2, step)
    scores = [(_projection_score(preview, float(angle)), float(angle)) for angle in angles]
    scores.sort(reverse=True)
    best_score, corrective = scores[0]
    zero_score = _projection_score(preview, 0.0)
    gain = (best_score - zero_score) / max(best_score, 1.0)
    if gain < 0.005:
        corrective = 0.0
        status = "observed-no-correction"
    else:
        status = "corrected"
    normalized = source.rotate(corrective, resample=Image.Resampling.BICUBIC,
                               fillcolor="white") if corrective else source.copy()
    report = {"status": status, "observed_skew_deg": round(-corrective, 3),
              "applied_rotation_deg": round(corrective, 3),
              "projection_gain": round(gain, 6), "perspective_status": "unknown",
              "perspective_reason": "four-page-borders-not-observed",
              "inverse_affine": _inverse_affine(source.width, source.height, corrective)}
    return normalized, report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    try:
        with Image.open(args.source) as image:
            normalized, report = normalize_image(image.convert("RGB"))
        args.output.parent.mkdir(parents=True, exist_ok=True)
        normalized.save(args.output)
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(json.dumps(report, indent=2) + "\n")
        print(json.dumps(report, indent=2))
        return 0
    except Exception as error:
        print(f"page-geometry-normalizer: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
