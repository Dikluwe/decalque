#!/usr/bin/env python3
"""Measure candidate PDF word ink after rendering it at scan resolution."""

from __future__ import annotations

import math
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Callable

import scan_word_compare
import typographic_profile


def render_page(
    candidate: Path, page_index: int, width: int, height: int
) -> Any:
    import cv2

    with tempfile.TemporaryDirectory(prefix="decalque-candidate-") as directory:
        prefix = Path(directory) / "page"
        process = subprocess.run(
            [
                "pdftocairo", "-png", "-singlefile", "-f", str(page_index + 1),
                "-l", str(page_index + 1), "-scale-to-x", str(width),
                "-scale-to-y", str(height), str(candidate), str(prefix),
            ],
            check=False, capture_output=True, text=True,
        )
        if process.returncode != 0:
            raise RuntimeError(process.stderr.strip() or "candidate rendering failed")
        image = cv2.imread(str(prefix.with_suffix(".png")), cv2.IMREAD_COLOR)
        if image is None:
            raise RuntimeError("candidate rendering produced no image")
        return image


def ink_bbox(image: Any, bounds: list[int]) -> list[int] | None:
    import numpy as np

    x0, y0, x1, y1 = bounds
    crop = image[y0:y1, x0:x1]
    if crop.size == 0:
        return None
    gray = (
        np.dot(crop[..., :3], [0.114, 0.587, 0.299]).astype(np.uint8)
        if crop.ndim == 3 else crop.astype(np.uint8)
    )
    histogram = np.bincount(gray.ravel(), minlength=256)
    total, weighted = gray.size, sum(value * count for value, count in enumerate(histogram))
    background_weight = background_sum = 0
    best_score, threshold = -1.0, 0
    for value, count in enumerate(histogram):
        background_weight += int(count)
        if background_weight == 0:
            continue
        foreground_weight = total - background_weight
        if foreground_weight == 0:
            break
        background_sum += value * int(count)
        background_mean = background_sum / background_weight
        foreground_mean = (weighted - background_sum) / foreground_weight
        score = background_weight * foreground_weight * (background_mean - foreground_mean) ** 2
        if score > best_score:
            best_score, threshold = score, value
    ys, xs = (gray <= threshold).nonzero()
    if len(xs) == 0:
        return None
    return [x0 + int(xs.min()), y0 + int(ys.min()), x0 + int(xs.max()) + 1, y0 + int(ys.max()) + 1]


def candidate_profiles(
    catalog: dict[str, Any], image: Any, width: int, height: int
) -> dict[str, list[dict[str, Any]]]:
    page = catalog.get("page", {})
    width_pt, height_pt = page.get("width_pt"), page.get("height_pt")
    if not width_pt or not height_pt:
        return {}
    sx, sy = width / width_pt, height / height_pt
    profiles: dict[str, list[dict[str, Any]]] = {}
    words = scan_word_compare.candidate_words(catalog.get("glyphs", []))
    line_baselines = {
        word["line_id"]: word["baseline_y"] for word in words
    }
    for word in words:
        baseline_px = word["baseline_y"] * sy
        margin_top = 1.5 * word["font_size_pt"] * sy
        margin_bottom = 0.6 * word["font_size_pt"] * sy
        previous_baseline = line_baselines.get(word["line_id"] - 1)
        next_baseline = line_baselines.get(word["line_id"] + 1)
        top = baseline_px - margin_top
        bottom = baseline_px + margin_bottom
        if previous_baseline is not None:
            top = max(top, (previous_baseline + word["baseline_y"]) * sy / 2)
        if next_baseline is not None:
            bottom = min(bottom, (next_baseline + word["baseline_y"]) * sy / 2)
        bounds = [
            max(0, math.floor(word["x0"] * sx)),
            max(0, math.floor(top)),
            min(width, math.ceil(word["x1"] * sx)),
            min(height, math.ceil(bottom)),
        ]
        bbox = ink_bbox(image, bounds)
        if bbox is None:
            profile = {"status": "unknown", "reason": "candidate-ink-absent"}
        else:
            segment = {
                "text": word["text"], "bbox": bbox,
                "bbox_pt": [bbox[0] / sx, bbox[1] / sy, bbox[2] / sx, bbox[3] / sy],
                "baseline_y_px": baseline_px, "baseline_y_pt": word["baseline_y"],
                "baseline_confidence": 1.0,
            }
            profile = typographic_profile.profile_for_segment(segment)
        profiles.setdefault(scan_word_compare.key(word["text"]), []).append(profile)
    return profiles


def enrich_page(
    page: dict[str, Any], catalog: dict[str, Any], candidate: Path, page_index: int,
    renderer: Callable[[Path, int, int, int], Any] = render_page,
) -> dict[str, Any]:
    width, height = int(page.get("width") or 0), int(page.get("height") or 0)
    if width <= 0 or height <= 0:
        return page
    profiles = candidate_profiles(catalog, renderer(candidate, page_index, width, height), width, height)
    for region in page.get("regions", []):
        for line in region.get("detected_lines", []):
            for segment in line.get("word_segments", []):
                matches = profiles.get(scan_word_compare.key(segment.get("text")), [])
                segment["candidate_typographic_profile"] = (
                    matches[0] if len(matches) == 1 else {
                        "status": "unknown", "reason": "candidate-word-ambiguous"
                    }
                )
    return page
