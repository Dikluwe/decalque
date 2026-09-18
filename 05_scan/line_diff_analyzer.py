#!/usr/bin/env python3
# Crystalline Lineage
# @prompt 00_nucleo/prompts/line-diff-analysis.md
# @layer L5
# @updated 2026-09-15
"""Align raster lines and emit localized layout/OCR review evidence."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image


WORDS = re.compile(r"\S+")


def ink_array(path: Path) -> np.ndarray:
    with Image.open(path) as source:
        return 1.0 - np.asarray(source.convert("L"), dtype=np.float64) / 255.0


def line_zones(ink: np.ndarray, threshold: float = 0.001, minimum_height: int = 4) -> list[dict[str, Any]]:
    active = ink.mean(axis=1) > threshold
    zones, start = [], None
    for y, present in enumerate(active):
        if present and start is None:
            start = y
        if start is not None and (not present or y == len(active) - 1):
            bottom = y if not present else y + 1
            if bottom - start >= minimum_height:
                crop = ink[start:bottom]
                columns = np.where(crop.max(axis=0) > 0.05)[0]
                if len(columns):
                    zones.append({"top": start, "bottom": bottom, "left": int(columns[0]),
                                  "right": int(columns[-1] + 1), "center_y": (start + bottom) / 2,
                                  "height": bottom - start, "width": int(columns[-1] + 1 - columns[0])})
            start = None
    return zones


def normalized_crop(ink: np.ndarray, zone: dict[str, Any], height: int = 32, width: int = 512) -> np.ndarray:
    crop = ink[zone["top"]:zone["bottom"], zone["left"]:zone["right"]]
    image = Image.fromarray(np.uint8(np.clip(1.0 - crop, 0, 1) * 255), mode="L")
    image.thumbnail((width, height), Image.Resampling.LANCZOS)
    canvas = Image.new("L", (width, height), 255)
    canvas.paste(image, (0, (height - image.height) // 2))
    return 1.0 - np.asarray(canvas, dtype=np.float64) / 255.0


def shape_error(ref_ink: np.ndarray, cand_ink: np.ndarray, left: dict, right: dict) -> float:
    reference, candidate = normalized_crop(ref_ink, left), normalized_crop(cand_ink, right)
    difference = np.abs(reference - candidate).sum()
    average_ink = (reference.sum() + candidate.sum()) / 2
    return float(difference / max(1.0, average_ink))


def align_lines(reference: list[dict], candidate: list[dict], ref_ink: np.ndarray,
                cand_ink: np.ndarray) -> list[tuple[int | None, int | None]]:
    rows, cols = len(reference), len(candidate)
    dp = np.full((rows + 1, cols + 1), np.inf); dp[0, 0] = 0
    back: dict[tuple[int, int], tuple[int, int]] = {}
    for i in range(rows + 1):
        for j in range(cols + 1):
            if i < rows and dp[i + 1, j] > dp[i, j] + 0.35:
                dp[i + 1, j], back[i + 1, j] = dp[i, j] + 0.35, (i, j)
            if j < cols and dp[i, j + 1] > dp[i, j] + 0.35:
                dp[i, j + 1], back[i, j + 1] = dp[i, j] + 0.35, (i, j)
            if i < rows and j < cols:
                vertical = abs(reference[i]["center_y"] / ref_ink.shape[0] -
                               candidate[j]["center_y"] / cand_ink.shape[0])
                cost = min(0.7, shape_error(ref_ink, cand_ink, reference[i], candidate[j]) * 3 + vertical)
                if dp[i + 1, j + 1] > dp[i, j] + cost:
                    dp[i + 1, j + 1], back[i + 1, j + 1] = dp[i, j] + cost, (i, j)
    result, i, j = [], rows, cols
    while i or j:
        pi, pj = back[i, j]
        result.append((pi if i > pi and j > pj else (pi if i > pi else None),
                       pj if i > pi and j > pj else (pj if j > pj else None)))
        i, j = pi, pj
    return list(reversed(result))


def word_boxes(ink: np.ndarray, zone: dict, minimum_gap: int) -> list[list[int]]:
    crop = ink[zone["top"]:zone["bottom"], zone["left"]:zone["right"]]
    active = crop.max(axis=0) > 0.05
    components, start = [], None
    for x, present in enumerate(active):
        if present and start is None: start = x
        if start is not None and (not present or x == len(active) - 1):
            components.append([start, x if not present else x + 1]); start = None
    groups = []
    for component in components:
        if groups and component[0] - groups[-1][1] < minimum_gap:
            groups[-1][1] = component[1]
        else: groups.append(component)
    return [[zone["left"] + a, zone["top"], zone["left"] + b, zone["bottom"]] for a, b in groups]


def classification(dx: float, width_ratio: float, height_ratio: float, raster_error: float) -> str:
    if abs(dx) > 3 and abs(width_ratio - 1) < 0.03: return "position"
    if abs(width_ratio - 1) > 0.06 and abs(height_ratio - 1) < 0.05: return "tracking_or_glyph_width"
    if abs(height_ratio - 1) > 0.06: return "font_size_or_weight"
    if raster_error > 0.12: return "localized_typography_or_ocr"
    return "preserved"


def analyze(reference_path: Path, candidate_path: Path, text_lines: list[str]) -> dict[str, Any]:
    ref_ink, cand_ink = ink_array(reference_path), ink_array(candidate_path)
    if ref_ink.shape != cand_ink.shape: raise ValueError("dimensões raster diferentes")
    refs, cands = line_zones(ref_ink), line_zones(cand_ink)
    aligned = align_lines(refs, cands, ref_ink, cand_ink)
    lines, reviews, text_index = [], [], 0
    for index, (ri, ci) in enumerate(aligned):
        if ri is None or ci is None:
            lines.append({"alignment": "candidate_only" if ri is None else "reference_only",
                          "reference_line": ri, "candidate_line": ci, "classification": "reflow"})
            continue
        ref, cand = refs[ri], cands[ci]
        text = text_lines[text_index] if text_index < len(text_lines) else None
        text_index += 1
        dx, dy = cand["left"] - ref["left"], cand["center_y"] - ref["center_y"]
        wr, hr = cand["width"] / ref["width"], cand["height"] / ref["height"]
        error = shape_error(ref_ink, cand_ink, ref, cand)
        kind = classification(dx, wr, hr, error)
        item = {"alignment": "paired", "reference_line": ri, "candidate_line": ci, "text": text,
                "delta_x_px": round(dx, 3), "delta_y_px": round(dy, 3),
                "width_ratio": round(wr, 5), "height_ratio": round(hr, 5),
                "normalized_raster_error": round(error, 6), "classification": kind}
        words = WORDS.findall(text or "")
        ref_words, cand_words = word_boxes(ref_ink, ref, max(3, int(ref["height"] * .18))), word_boxes(cand_ink, cand, max(3, int(cand["height"] * .18)))
        item["word_geometry_status"] = "observed" if len(words) == len(ref_words) == len(cand_words) else "unknown"
        if kind == "localized_typography_or_ocr" and item["word_geometry_status"] == "observed":
            for word, rb, cb in zip(words, ref_words, cand_words):
                rz = {"left": rb[0], "top": rb[1], "right": rb[2], "bottom": rb[3]}
                cz = {"left": cb[0], "top": cb[1], "right": cb[2], "bottom": cb[3]}
                score = shape_error(ref_ink, cand_ink, rz, cz)
                if score > 0.12:
                    review = {"status": "review_required", "line": index, "word": word,
                              "reason": "localized-raster-outlier", "visual_difference": round(score, 6)}
                    reviews.append(review)
        lines.append(item)
    return {"schema_version": 1, "status": "observed", "reference": str(reference_path.resolve()),
            "candidate": str(candidate_path.resolve()), "reference_zone_count": len(refs),
            "candidate_zone_count": len(cands), "lines": lines, "ocr_review_queue": reviews,
            "summary": {"paired": sum(x["alignment"] == "paired" for x in lines),
                        "reflow": sum(x["classification"] == "reflow" for x in lines),
                        "review_required": len(reviews)}}


def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("reference", type=Path); parser.add_argument("candidate", type=Path)
    parser.add_argument("--text-file", type=Path)
    args = parser.parse_args()
    try:
        texts = args.text_file.read_text().splitlines() if args.text_file else []
        json.dump(analyze(args.reference, args.candidate, texts), sys.stdout, ensure_ascii=False, indent=2); sys.stdout.write("\n"); return 0
    except Exception as error:
        print(f"line-diff-analyzer: {error}", file=sys.stderr); return 2


if __name__ == "__main__": raise SystemExit(main())
