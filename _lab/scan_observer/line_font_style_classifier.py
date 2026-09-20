#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/line-font-style-classification.md
# @updated 2026-09-17
"""Classify dominant line weight and slant by raster candidate comparison."""

from __future__ import annotations

import argparse
import copy
import json
import statistics
import sys
from collections import Counter
from pathlib import Path
from typing import Any

from PIL import Image, ImageDraw, ImageFilter, ImageFont


CANDIDATES = ("light", "regular", "italic", "bold", "bold-italic")
WEIGHT_CANDIDATES = ("light", "regular", "bold")


def render_candidate_mask(text: str, regular: Path, size: int, style: str,
                          light: Path | None = None, bold: Path | None = None) -> Image.Image:
    font_path = (light if style == "light" and light else
                 bold if style in {"bold", "bold-italic"} and bold else regular)
    font = ImageFont.truetype(str(font_path), size)
    left, top, right, bottom = font.getbbox(text, stroke_width=0)
    padding = max(6, size // 3)
    canvas = Image.new("L", (max(1, right - left + padding * 2),
                             max(1, bottom - top + padding * 2)), 0)
    draw = ImageDraw.Draw(canvas)
    stroke = (max(1, round(size * .045))
              if style in {"bold", "bold-italic"} and not bold else 0)
    draw.text((padding - left, padding - top), text, font=font, fill=255,
              stroke_width=stroke, stroke_fill=255)
    if style in {"italic", "bold-italic"}:
        shear = 0.22
        extra = round(canvas.height * shear) + 2
        canvas = canvas.transform((canvas.width + extra, canvas.height), Image.Transform.AFFINE,
                                  (1, -shear, extra, 0, 1, 0),
                                  resample=Image.Resampling.BICUBIC)
    bbox = canvas.getbbox()
    return canvas.crop(bbox) if bbox else canvas


def source_mask(crop: Image.Image) -> Image.Image | None:
    gray = crop.convert("L")
    histogram = gray.histogram()
    total = sum(histogram)
    weighted = sum(index * count for index, count in enumerate(histogram))
    back_count = back_sum = 0
    best, threshold = -1.0, 180
    for value, count in enumerate(histogram):
        back_count += count
        back_sum += value * count
        front_count = total - back_count
        if not back_count or not front_count:
            continue
        difference = back_sum / back_count - (weighted - back_sum) / front_count
        score = back_count * front_count * difference * difference
        if score > best:
            best, threshold = score, value
    mask = gray.point(lambda value: 255 if value <= threshold else 0)
    bbox = mask.getbbox()
    return mask.crop(bbox) if bbox else None


def mask_error(source: Image.Image, candidate: Image.Image) -> float:
    candidate = candidate.resize(source.size, Image.Resampling.LANCZOS).point(
        lambda value: 255 if value >= 128 else 0)
    source = source.point(lambda value: 255 if value >= 128 else 0)
    source_dilated = source.filter(ImageFilter.MaxFilter(3))
    candidate_dilated = candidate.filter(ImageFilter.MaxFilter(3))
    source_pixels = [value > 0 for value in source.getdata()]
    candidate_pixels = [value > 0 for value in candidate.getdata()]
    source_near = [value > 0 for value in source_dilated.getdata()]
    candidate_near = [value > 0 for value in candidate_dilated.getdata()]
    source_count, candidate_count = sum(source_pixels), sum(candidate_pixels)
    if not source_count or not candidate_count:
        return 1.0
    source_coverage = sum(value and candidate_near[index]
                          for index, value in enumerate(source_pixels)) / source_count
    candidate_coverage = sum(value and source_near[index]
                             for index, value in enumerate(candidate_pixels)) / candidate_count
    density_penalty = abs(source_count - candidate_count) / max(source_count, candidate_count)
    return (2 - source_coverage - candidate_coverage) / 2 + density_penalty * .25


def measure_slant(mask: Image.Image) -> dict[str, Any]:
    """Measure italic shear without normalizing away the glyph geometry.

    Vertical strokes in roman text overlap best with the row directly below them.
    In italic text the best overlap occurs after a small horizontal displacement.
    Comparing rows several pixels apart makes that sub-pixel shear observable.
    """
    binary = mask.point(lambda value: 255 if value >= 128 else 0)
    width, height = binary.size
    if width < 8 or height < 8:
        return {"status": "unknown", "reason": "insufficient-ink-geometry"}
    pixels = [value > 0 for value in binary.getdata()]
    row_delta = max(3, min(8, round(height * .14)))
    scores: dict[int, float] = {}
    for shift in range(-4, 5):
        overlap = union = 0
        for y in range(height - row_delta):
            upper = y * width
            lower = (y + row_delta) * width
            for x in range(width):
                shifted_x = x + shift
                if 0 <= shifted_x < width:
                    first = pixels[upper + x]
                    second = pixels[lower + shifted_x]
                    overlap += first and second
                    union += first or second
        scores[shift] = overlap / union if union else 0.0
    best_shift = max(scores, key=scores.get)
    gain = scores[best_shift] - scores[0]
    italic = best_shift != 0 and gain >= .008
    return {
        "status": "observed",
        "style": "italic" if italic else "normal",
        "best_shift_px": best_shift,
        "row_delta_px": row_delta,
        "gain_over_upright": round(gain, 6),
        "scores": {str(key): round(value, 6) for key, value in scores.items()},
        "source": "row-continuity-shear",
    }


def classify_crop(crop: Image.Image, text: str, regular: Path, light: Path,
                  minimum_margin: float = .004, bold: Path | None = None) -> dict[str, Any]:
    if not text.strip() or not regular.is_file() or not light.is_file():
        return {"status": "unknown", "reason": "missing-text-or-font"}
    observed = source_mask(crop)
    if observed is None:
        return {"status": "unknown", "reason": "ink-absent"}
    slant = measure_slant(observed)
    scores = {style: mask_error(observed, render_candidate_mask(
              text, regular, 64, style, light, bold)) for style in WEIGHT_CANDIDATES}
    ranked = sorted(scores, key=scores.get)
    margin = scores[ranked[1]] - scores[ranked[0]]
    if margin < minimum_margin:
        return {"status": "unknown", "reason": "candidate-margin-insufficient",
                "candidate_scores": scores, "best_candidate": ranked[0],
                "candidate_margin": margin, "slant_measurement": slant}
    weight = ranked[0]
    italic = slant.get("style") == "italic"
    winner = f"{weight}-italic" if italic and weight != "regular" else (
        "italic" if italic else weight)
    return {
        "status": "observed", "style_class": winner,
        "style": "italic" if italic else "normal",
        "weight": weight,
        "confidence": min(1.0, margin / .06), "candidate_margin": margin,
        "candidate_scores": {name: round(scores[name], 6) for name in WEIGHT_CANDIDATES},
        "slant_measurement": slant,
        "source": "geometric-slant-and-same-text-weight-comparison",
    }


def consolidate_region_styles(lines: list[dict[str, Any]],
                              strong_margin: float = .04,
                              support_ratio: float = .60) -> dict[str, Any]:
    """Turn noisy line observations into one conservative region decision."""
    observed = [line for line in lines
                if line.get("font_style", {}).get("status") == "observed"]
    if not observed:
        return {"status": "unknown", "weight": None, "slant": None,
                "observed_lines": 0, "total_lines": len(lines)}

    raw_classes = Counter(line["font_style"]["style_class"] for line in observed)
    italic_votes = sum(line["font_style"]["slant_measurement"].get("style") == "italic"
                       for line in observed)
    italic_ratio = italic_votes / len(observed)
    slant = "italic" if italic_ratio >= .5 else "normal"

    strong_support: dict[str, int] = {"light": 0, "bold": 0}
    advantages: dict[str, list[float]] = {"light": [], "bold": []}
    for line in observed:
        scores = line["font_style"]["candidate_scores"]
        for weight in ("light", "bold"):
            advantage = scores["regular"] - scores[weight]
            advantages[weight].append(advantage)
            if advantage >= strong_margin:
                strong_support[weight] += 1

    required = 1 if len(observed) == 1 else max(2, int(len(observed) * support_ratio + .9999))
    eligible = [weight for weight in ("light", "bold")
                if strong_support[weight] >= required]
    weight = (max(eligible, key=lambda name: sum(advantages[name]) / len(advantages[name]))
              if eligible else "regular")
    style_class = ("italic" if slant == "italic" and weight == "regular"
                   else f"{weight}-italic" if slant == "italic" else weight)

    for line in observed:
        style = line["font_style"]
        style["line_evidence"] = {
            "weight": style["weight"], "style": style["style"],
            "style_class": style["style_class"],
            "candidate_scores": copy.deepcopy(style["candidate_scores"]),
            "slant_measurement": copy.deepcopy(style["slant_measurement"]),
        }
        style["weight"] = weight
        style["style"] = slant
        style["style_class"] = style_class
        style["decision_scope"] = "region-consensus"

    inferred = []
    inheritable_reasons = {"candidate-margin-insufficient", "ink-absent",
                           "insufficient-ink-geometry"}
    for line in lines:
        style = line.get("font_style", {})
        if style.get("status") != "unknown" or style.get("reason") not in inheritable_reasons:
            continue
        original = copy.deepcopy(style)
        style.update({
            "status": "inferred", "weight": weight, "style": slant,
            "style_class": style_class, "decision_scope": "region-consensus",
            "inference_source": "region-consensus", "line_evidence": original,
        })
        inferred.append(line)

    return {
        "status": "observed", "dominant": style_class,
        "weight": weight, "slant": slant,
        "distribution": {style_class: len(observed) + len(inferred)},
        "raw_distribution": dict(raw_classes),
        "strong_weight_support": strong_support,
        "required_strong_support": required,
        "italic_vote_ratio": round(italic_ratio, 4),
        "observed_lines": len(observed), "total_lines": len(lines),
        "inferred_lines": len(inferred),
    }


def repair_region_line_boxes(image: Image.Image, lines: list[dict[str, Any]]) -> int:
    """Remove isolated left ink only when sibling alignment and a large gap agree."""
    valid = [line for line in lines if len(line.get("bbox", [])) == 4]
    if len(valid) < 3:
        return 0
    expected_left = round(statistics.median(float(line["bbox"][0]) for line in valid))
    repaired = 0
    gray = image.convert("L")
    for line in valid:
        original = [int(round(value)) for value in line["bbox"]]
        left, top, right, bottom = original
        height = max(1, bottom - top)
        if expected_left - left <= max(12, round(height * 1.25)):
            continue
        crop = gray.crop((left, top, min(right, expected_left + height), bottom))
        width, crop_height = crop.size
        pixels = list(crop.getdata())
        active = []
        for x in range(width):
            ink = sum(pixels[y * width + x] < 200 for y in range(crop_height))
            if ink >= 2:
                active.append(x)
        if len(active) < 2:
            continue
        gaps = [(active[index + 1] - active[index], active[index], active[index + 1])
                for index in range(len(active) - 1)]
        gap, _, text_start = max(gaps)
        absolute_start = left + text_start
        if gap < max(10, round(height * 1.25)):
            continue
        if abs(absolute_start - expected_left) > height:
            continue
        corrected_left = max(left, absolute_start - 2)
        line["bbox_original"] = list(line["bbox"])
        line["bbox"] = [corrected_left, line["bbox"][1], line["bbox"][2], line["bbox"][3]]
        line["bbox_repair"] = {
            "reason": "isolated-left-ink", "expected_left": expected_left,
            "detected_text_left": absolute_start, "discarded_gap_px": gap,
        }
        repaired += 1
    return repaired


def recompute_text_region_bbox(region: dict[str, Any]) -> bool:
    """Rebuild aggregate geometry so repaired line boxes become authoritative."""
    boxes = [line["bbox"] for line in region.get("lines", [])
             if len(line.get("bbox", [])) == 4]
    if not boxes:
        return False
    recomputed = [min(box[0] for box in boxes), min(box[1] for box in boxes),
                  max(box[2] for box in boxes), max(box[3] for box in boxes)]
    original = region.get("bbox")
    if original == recomputed:
        return False
    region["bbox_original"] = copy.deepcopy(original)
    region["bbox"] = recomputed
    region["bbox_repair"] = {
        "reason": "recomputed-from-clean-lines",
        "line_count": len(boxes),
    }
    return True


def classify_page(zones: dict[str, Any], image: Image.Image, regular: Path,
                  light: Path, bold: Path | None = None) -> dict[str, Any]:
    output = copy.deepcopy(zones)
    for region in output.get("regions", []):
        repaired_boxes = repair_region_line_boxes(image, region.get("lines", []))
        region_bbox_repaired = recompute_text_region_bbox(region)
        for line in region.get("lines", []):
            box = [int(round(value)) for value in line["bbox"]]
            line["font_style"] = classify_crop(image.crop(tuple(box)), line.get("text", ""),
                                                 regular, light, bold=bold)
        region["font_style_summary"] = consolidate_region_styles(region.get("lines", []))
        region["font_style_summary"]["repaired_line_boxes"] = repaired_boxes
        region["font_style_summary"]["region_bbox_recomputed"] = region_bbox_repaired
    return output


def render_overlay(image: Image.Image, result: dict[str, Any], path: Path) -> None:
    output = image.convert("RGB")
    draw = ImageDraw.Draw(output)
    colors = {"light": "#2980b9", "regular": "#1e8449", "italic": "#8e44ad",
              "bold": "#c0392b", "light-italic": "#6c5ce7",
              "bold-italic": "#d35400", "unknown": "#7f8c8d"}
    for region in result.get("regions", []):
        for line in region.get("lines", []):
            style = line.get("font_style", {})
            label = style.get("style_class", "unknown")
            color = colors[label]
            draw.rectangle(line["bbox"], outline=color, width=2)
            draw.text((line["bbox"][0] + 2, max(0, line["bbox"][1] - 11)), label,
                      fill=color, stroke_width=2, stroke_fill="white")
    path.parent.mkdir(parents=True, exist_ok=True)
    output.save(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("zones", type=Path)
    parser.add_argument("image", type=Path)
    parser.add_argument("--regular", required=True, type=Path)
    parser.add_argument("--light", required=True, type=Path)
    parser.add_argument("--bold", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--overlay", required=True, type=Path)
    args = parser.parse_args()
    try:
        zones = json.loads(args.zones.read_text(encoding="utf-8"))
        with Image.open(args.image).convert("RGB") as image:
            result = classify_page(zones, image, args.regular, args.light, args.bold)
            render_overlay(image, result, args.overlay)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
        print(json.dumps({"status": "success", "output": str(args.output.resolve()),
                          "overlay": str(args.overlay.resolve())}, ensure_ascii=False))
        return 0
    except Exception as error:
        print(f"line-font-style-classifier: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
