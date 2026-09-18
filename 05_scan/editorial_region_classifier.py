#!/usr/bin/env python3
# Crystalline Lineage
# @prompt 00_nucleo/prompts/editorial-region-classification.md
# @layer L5
# @updated 2026-09-17
"""Classify editorial roles from consolidated region typography and geometry."""

from __future__ import annotations

import argparse
import copy
import json
import re
import statistics
import sys
from pathlib import Path
from typing import Any

from PIL import Image, ImageDraw


COLORS = {
    "heading": "#d35400", "body_paragraph": "#1e8449", "epigraph": "#2471a3",
    "verse_or_title_list": "#8e44ad", "block_quote": "#6c5ce7",
    "biographical_note": "#a04000", "page_number": "#117864",
    "figure_caption": "#c0392b", "visual": "#229954", "unknown": "#7f8c8d",
}


def page_dimensions(page: dict[str, Any]) -> tuple[float, float]:
    width = page.get("width_px", page.get("width"))
    height = page.get("height_px", page.get("height"))
    if not width or not height:
        raise ValueError("dimensões da página ausentes")
    return float(width), float(height)


def region_features(region: dict[str, Any], page: dict[str, Any],
                    neighbours: list[dict[str, Any]]) -> dict[str, Any]:
    width, height = page_dimensions(page)
    lines = [line for line in region.get("lines", []) if len(line.get("bbox", [])) == 4]
    box = region.get("bbox", [0, 0, 0, 0])
    line_widths = [float(line["bbox"][2]) - float(line["bbox"][0]) for line in lines]
    lefts = [float(line["bbox"][0]) for line in lines]
    median_width = statistics.median(line_widths) if line_widths else 0.0
    width_deviation = statistics.pstdev(line_widths) if len(line_widths) > 1 else 0.0
    style = region.get("font_style_summary", {})
    text = " ".join(str(line.get("text", "")) for line in lines)
    before = after = None
    ordered = sorted((item for item in neighbours if len(item.get("bbox", [])) == 4),
                     key=lambda item: item["bbox"][1])
    for index, item in enumerate(ordered):
        if item.get("id") != region.get("id"):
            continue
        if index:
            before = max(0.0, float(box[1]) - float(ordered[index - 1]["bbox"][3]))
        if index + 1 < len(ordered):
            after = max(0.0, float(ordered[index + 1]["bbox"][1]) - float(box[3]))
        break
    return {
        "line_count": len(lines),
        "median_line_width_ratio": round(median_width / width, 4),
        "maximum_line_width_ratio": round(max(line_widths, default=0.0) / width, 4),
        "line_width_variation_ratio": round(width_deviation / width, 4),
        "left_margin_ratio": round(statistics.median(lefts) / width, 4) if lefts else None,
        "left_margin_spread_ratio": round((max(lefts) - min(lefts)) / width, 4) if lefts else None,
        "top_ratio": round(float(box[1]) / height, 4) if len(box) == 4 else None,
        "bottom_ratio": round(float(box[3]) / height, 4) if len(box) == 4 else None,
        "space_before_ratio": round(before / height, 4) if before is not None else None,
        "space_after_ratio": round(after / height, 4) if after is not None else None,
        "slant": style.get("slant"), "weight": style.get("weight"),
        "typography_status": style.get("status", "unknown"),
        "text": text,
    }


def classify_region(region: dict[str, Any], page: dict[str, Any],
                    neighbours: list[dict[str, Any]]) -> dict[str, Any]:
    source_type = region.get("type", "unknown")
    if source_type == "visual":
        return {"status": "observed", "role": "visual", "confidence": 1.0,
                "source_region_type": source_type, "rules": ["observed-visual"],
                "features": region_features(region, page, neighbours)}
    features = region_features(region, page, neighbours)
    count = features["line_count"]
    if not count:
        return {"status": "unknown", "role": "unknown", "confidence": 0.0,
                "source_region_type": source_type, "rules": ["no-text-lines"],
                "features": features}
    if source_type == "page-number":
        role, confidence, rules = "page_number", .99, ["page-number-geometry"]
    elif (features["weight"] == "bold" and count <= 4
          and features["maximum_line_width_ratio"] < .60
          and re.search(r"\bfigure\b", features["text"], flags=re.IGNORECASE)):
        role, confidence, rules = "figure_caption", .94, [
            "short-bold-block", "figure-label-support"]
    elif source_type == "heading" and features["top_ratio"] < .18 and count <= 3:
        role, confidence, rules = "heading", .92, ["upper-heading-geometry"]
    elif (count <= 2 and features["top_ratio"] < .4
          and features["median_line_width_ratio"] < .65
          and (features["left_margin_ratio"] or 0) > .07):
        role, confidence, rules = "epigraph", .88, ["short-indented-detached-block"]
    elif features["slant"] == "italic":
        text = features["text"].casefold()
        bio_terms = re.findall(
            r"\b(?:author|president|member|committee|professor|director|born|books?|papers?)\b",
            text,
        )
        if (features["top_ratio"] > .68 and features["median_line_width_ratio"] > .55
                and len(set(bio_terms)) >= 2):
            role, confidence, rules = "biographical_note", .91, [
                "bottom-italic-prose", "biographical-text-support"]
        elif (count >= 3 and features["median_line_width_ratio"] < .55
              and features["maximum_line_width_ratio"] < .68):
            role, confidence, rules = "verse_or_title_list", .90, [
                "italic-short-variable-lines"]
        else:
            role, confidence, rules = "block_quote", .82, ["italic-prose-block"]
    elif count >= 2:
        role, confidence, rules = "body_paragraph", .90, [
            "regular-filled-lines" if features["median_line_width_ratio"] >= .50
            else "regular-narrow-column"]
    else:
        role, confidence, rules = "unknown", .0, ["insufficient-editorial-evidence"]
    return {"status": "observed" if role != "unknown" else "unknown", "role": role,
            "confidence": confidence, "source_region_type": source_type,
            "rules": rules, "features": features}


def classify_page(payload: dict[str, Any]) -> dict[str, Any]:
    output = copy.deepcopy(payload)
    regions = output.get("regions", [])
    page = output.get("page", {})
    for region in regions:
        region["editorial_role"] = classify_region(region, page, regions)
    output["editorial_summary"] = {
        "classified_regions": sum(region["editorial_role"]["status"] == "observed"
                                  for region in regions),
        "unknown_regions": sum(region["editorial_role"]["status"] == "unknown"
                               for region in regions),
    }
    return output


def render_overlay(image: Image.Image, payload: dict[str, Any], path: Path) -> None:
    output = image.convert("RGB")
    draw = ImageDraw.Draw(output)
    for region in payload.get("regions", []):
        editorial = region.get("editorial_role", {})
        role = editorial.get("role", "unknown")
        color = COLORS.get(role, COLORS["unknown"])
        draw.rectangle(region["bbox"], outline=color, width=3)
        draw.text((region["bbox"][0] + 3, max(0, region["bbox"][1] - 13)),
                  f'{region.get("id", "?")} {role}', fill=color,
                  stroke_width=2, stroke_fill="white")
    path.parent.mkdir(parents=True, exist_ok=True)
    output.save(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("regions", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--image", type=Path)
    parser.add_argument("--overlay", type=Path)
    args = parser.parse_args()
    try:
        payload = json.loads(args.regions.read_text(encoding="utf-8"))
        result = classify_page(payload)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n",
                               encoding="utf-8")
        if args.image and args.overlay:
            with Image.open(args.image) as image:
                render_overlay(image, result, args.overlay)
        print(json.dumps({"status": "success", "output": str(args.output.resolve()),
                          "summary": result["editorial_summary"]}, ensure_ascii=False))
        return 0
    except Exception as error:
        print(f"editorial-region-classifier: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
