#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/page-zone-consensus.md
# @updated 2026-09-16
"""Build hierarchical page zones from observed ink and independent OCR witnesses."""

from __future__ import annotations

import argparse
import difflib
import json
import math
import re
import statistics
import sys
from pathlib import Path
from typing import Any

from PIL import Image, ImageDraw, ImageFont

import word_geometry_detector


COLORS = {
    "heading": "#e67e22",
    "epigraph": "#8e44ad",
    "paragraph": "#2471a3",
    "list": "#7d3c98",
    "note": "#566573",
    "page-number": "#117864",
    "visual": "#229954",
    "unknown": "#c0392b",
}


def normalized(value: str) -> str:
    value = re.sub(r"<img\b[^>]*>", " ", value, flags=re.IGNORECASE)
    value = re.sub(r"<[^>]+>", " ", value)
    value = re.sub(r"^[#>*+\-]+\s*", "", value, flags=re.MULTILINE)
    return " ".join(re.findall(r"\w+(?:['’]\w+)?", value.casefold(), re.UNICODE))


def witness_lines(payload: dict[str, Any]) -> list[str]:
    provider = payload.get("providers", {}).get("text_witness", {})
    return [line.strip() for line in str(provider.get("text", "")).splitlines()
            if line.strip()]


def ink_bands(image: Image.Image, threshold: int = 180) -> list[dict[str, Any]]:
    gray = image.convert("L")
    width, height = gray.size
    minimum_row_ink = max(3, round(width * 0.005))
    rows = [sum(gray.getpixel((x, y)) < threshold for x in range(width)) >= minimum_row_ink
            for y in range(height)]
    raw = []
    start = None
    for y, active in enumerate(rows + [False]):
        if active and start is None:
            start = y
        elif not active and start is not None:
            raw.append([start, y])
            start = None
    merge_gap = max(2, math.ceil(height * 0.0025))
    merged: list[list[int]] = []
    for y0, y1 in raw:
        if merged and y0 - merged[-1][1] <= merge_gap:
            merged[-1][1] = y1
        else:
            merged.append([y0, y1])
    bands = []
    for y0, y1 in merged:
        points = [(x, y) for y in range(y0, y1) for x in range(width)
                  if gray.getpixel((x, y)) < threshold]
        if not points:
            continue
        xs = [point[0] for point in points]
        ys = [point[1] for point in points]
        bands.append({"bbox": [min(xs), min(ys), max(xs) + 1, max(ys) + 1]})
    return bands


def split_text_and_visual_bands(
    bands: list[dict[str, Any]], line_count: int, width: int, height: int
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    if len(bands) == line_count:
        return bands, []
    textual = [band for band in bands
               if band["bbox"][2] - band["bbox"][0] >= width * 0.12
               or band["bbox"][1] >= height * 0.85]
    visuals = [band for band in bands if band not in textual]
    if len(textual) != line_count:
        raise ValueError(
            f"contagem incompatível: {len(textual)} linhas de tinta para {line_count} linhas OCR"
        )
    return textual, visuals


def quadrants(width: int, height: int) -> list[dict[str, Any]]:
    middle_x, middle_y = width // 2, height // 2
    return [
        {"id": "Q1", "bbox": [0, 0, middle_x, middle_y]},
        {"id": "Q2", "bbox": [middle_x, 0, width, middle_y]},
        {"id": "Q3", "bbox": [0, middle_y, middle_x, height]},
        {"id": "Q4", "bbox": [middle_x, middle_y, width, height]},
    ]


def intersects(left: list[int], right: list[int]) -> bool:
    return left[0] < right[2] and left[2] > right[0] and left[1] < right[3] and left[3] > right[1]


def touched_quadrants(bbox: list[int], page_quadrants: list[dict[str, Any]]) -> list[str]:
    return [item["id"] for item in page_quadrants if intersects(bbox, item["bbox"])]


def classify_region(
    bbox: list[int], lines: list[dict[str, Any]], width: int, height: int
) -> str:
    x0, y0, x1, _ = bbox
    if y0 >= height * 0.85 and x1 - x0 < width * 0.2:
        return "page-number"
    if len(lines) == 1 and x0 > width * 0.08:
        return "epigraph"
    if y0 < height * 0.14:
        return "heading"
    if y0 > height * 0.7 and x0 > width * 0.08:
        return "note"
    median_left = statistics.median(line["bbox"][0] for line in lines)
    if len(lines) >= 3 and median_left > width * 0.08 and x1 < width * 0.92:
        return "list"
    return "paragraph"


def provider_support(payload: dict[str, Any], region_text: str) -> tuple[list[str], list[str]]:
    target = normalized(region_text)
    present, supporting = [], []
    mapping = {
        "ovisocr2": payload.get("providers", {}).get("layout"),
        "paddleocr": payload.get("providers", {}).get("text_witness"),
        "got_ocr2": payload.get("providers", {}).get("got_ocr2"),
    }
    for name, provider in mapping.items():
        if not isinstance(provider, dict) or not provider.get("text"):
            continue
        present.append(name)
        evidence = normalized(str(provider["text"]))
        if target and (target in evidence or difflib.SequenceMatcher(
                None, target, evidence, autojunk=False).quick_ratio() >= 0.9):
            supporting.append(name)
    return present, supporting


def consensus_status(geometry: bool, present: list[str], supporting: list[str]) -> str:
    if geometry and len(supporting) >= 2:
        return "confirmed"
    if len(present) >= 2 and not supporting:
        return "disputed"
    if geometry:
        return "geometry-confirmed"
    if len(supporting) >= 2:
        return "text-confirmed"
    return "unknown"


def build(image: Image.Image, payload: dict[str, Any]) -> dict[str, Any]:
    width, height = image.size
    lines_text = witness_lines(payload)
    if not lines_text:
        raise ValueError("testemunho textual não contém linhas")
    supplied_geometry = payload.get("line_geometry")
    if supplied_geometry:
        if len(supplied_geometry) != len(lines_text):
            raise ValueError("contagem incompatível entre line_geometry e testemunho textual")
        text_bands = [{"bbox": list(item["bbox"])} for item in supplied_geometry]
        visual_bands = [{"bbox": list(asset["bbox"])} for asset in payload.get("assets", [])]
    else:
        text_bands, visual_bands = split_text_and_visual_bands(
            ink_bands(image), len(lines_text), width, height
        )
    page_quadrants = quadrants(width, height)
    gray = image.convert("L")
    lines = []
    for index, (text, band) in enumerate(zip(lines_text, text_bands)):
        x0, y0, x1, y1 = band["bbox"]
        ink = [[gray.getpixel((x, y)) < 180 for x in range(x0, x1)]
               for y in range(y0, y1)]
        minimum_gap = max(2, round((y1 - y0) * 0.18))
        anchors = word_geometry_detector.margin_word_anchors(
            ink, text, [x0, y0, x1, y1], minimum_gap
        )
        lines.append({"id": f"L{index + 1:03d}", "text": text, "bbox": band["bbox"],
                      **anchors})

    heights = [line["bbox"][3] - line["bbox"][1] for line in lines]
    gaps = [lines[index + 1]["bbox"][1] - lines[index]["bbox"][3]
            for index in range(len(lines) - 1)]
    region_gap = max(statistics.median(heights) * 0.6,
                     statistics.median(gaps) * 2 if gaps else 0)
    groups: list[list[dict[str, Any]]] = []
    for line in lines:
        candidates = []
        for group in groups:
            group_bottom = max(item["bbox"][3] for item in group)
            vertical_gap = line["bbox"][1] - group_bottom
            previous = group[-1]["bbox"]
            overlap = max(0, min(previous[2], line["bbox"][2]) - max(previous[0], line["bbox"][0]))
            minimum_width = min(previous[2] - previous[0], line["bbox"][2] - line["bbox"][0])
            if -max(heights) <= vertical_gap <= region_gap and overlap >= minimum_width * .2:
                candidates.append((overlap / max(1, minimum_width), -abs(vertical_gap), group))
        if candidates:
            max(candidates, key=lambda item: (item[0], item[1]))[2].append(line)
        else:
            groups.append([line])
    groups.sort(key=lambda group: (min(item["bbox"][1] for item in group),
                                   min(item["bbox"][0] for item in group)))

    regions = []
    for index, group in enumerate(groups, 1):
        bbox = [min(line["bbox"][0] for line in group), min(line["bbox"][1] for line in group),
                max(line["bbox"][2] for line in group), max(line["bbox"][3] for line in group)]
        region_text = "\n".join(line["text"] for line in group)
        present, supporting = provider_support(payload, region_text)
        kind = classify_region(bbox, group, width, height)
        regions.append({
            "id": f"R{index:03d}", "type": kind, "bbox": bbox,
            "quadrants": touched_quadrants(bbox, page_quadrants), "lines": group,
            "providers_present": present, "providers_supporting": supporting,
            "consensus_status": consensus_status(True, present, supporting),
        })
    for band in visual_bands:
        index = len(regions) + 1
        bbox = band["bbox"]
        regions.append({"id": f"R{index:03d}", "type": "visual", "bbox": bbox,
                        "quadrants": touched_quadrants(bbox, page_quadrants), "lines": [],
                        "providers_present": [], "providers_supporting": [],
                        "consensus_status": "geometry-confirmed"})
    providers = payload.get("providers", {})
    return {
        "schema_version": 1,
        "page": {"width_px": width, "height_px": height},
        "providers": {
            "ovisocr2": providers.get("layout", {}).get("model", "unavailable"),
            "paddleocr": providers.get("text_witness", {}).get("model", "unavailable"),
            "got_ocr2": providers.get("got_ocr2", {}).get("model", "unavailable"),
        },
        "quadrants": page_quadrants,
        "regions": regions,
    }


def render_overlay(image: Image.Image, result: dict[str, Any], path: Path) -> None:
    output = image.convert("RGBA")
    veil = Image.new("RGBA", output.size, (255, 255, 255, 105))
    output = Image.alpha_composite(output, veil)
    draw = ImageDraw.Draw(output)
    font = ImageFont.load_default()
    width, height = output.size
    draw.line((width // 2, 0, width // 2, height), fill="#95a5a6", width=2)
    draw.line((0, height // 2, width, height // 2), fill="#95a5a6", width=2)
    for region in result["regions"]:
        color = COLORS.get(region["type"], COLORS["unknown"])
        draw.rectangle(region["bbox"], outline=color, width=3)
        draw.text((region["bbox"][0] + 3, region["bbox"][1] + 2),
                  f'{region["id"]} {region["type"]} {region["consensus_status"]}',
                  fill=color, font=font, stroke_width=2, stroke_fill="white")
        for line in region["lines"]:
            draw.rectangle(line["bbox"], outline="#3498db", width=1)
            if line.get("margin_anchor_status") == "observed":
                draw.rectangle(line["left"]["bbox"], outline="#00a86b", width=2)
                draw.rectangle(line["right"]["bbox"], outline="#d35400", width=2)
    path.parent.mkdir(parents=True, exist_ok=True)
    output.convert("RGB").save(path)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("ocr", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--overlay", required=True, type=Path)
    args = parser.parse_args()
    try:
        payload = json.loads(args.ocr.read_text(encoding="utf-8"))
        with Image.open(args.image) as image:
            result = build(image.convert("RGB"), payload)
            render_overlay(image, result, args.overlay)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n",
                               encoding="utf-8")
        json.dump({"status": "success", "regions": len(result["regions"]),
                   "output": str(args.output.resolve()), "overlay": str(args.overlay.resolve())},
                  sys.stdout, ensure_ascii=False)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"page-zone-marker: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
