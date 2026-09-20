#!/usr/bin/env python3
"""Read each observed physical line independently through LM Studio."""

from __future__ import annotations

import argparse
import json
import statistics
import sys
import tempfile
from pathlib import Path
from typing import Any

from PIL import Image, ImageDraw

import dual_lmstudio_ocr


def raw_bands(image: Image.Image, y_offset: int = 0, x_offset: int = 0) -> list[list[int]]:
    gray = image.convert("L")
    width, height = gray.size
    active = [sum(gray.getpixel((x, y)) < 180 for x in range(width)) >= max(3, round(width * .005))
              for y in range(height)]
    runs, start = [], None
    for y, on in enumerate(active + [False]):
        if on and start is None:
            start = y
        elif not on and start is not None:
            points = [(x, yy) for yy in range(start, y) for x in range(width)
                      if gray.getpixel((x, yy)) < 180]
            if points:
                xs, ys = zip(*points)
                runs.append([x_offset + min(xs), y_offset + min(ys),
                             x_offset + max(xs) + 1, y_offset + max(ys) + 1])
            start = None
    return runs


def detect_lines(image: Image.Image, assets: list[dict[str, Any]]) -> list[list[int]]:
    masked = image.convert("RGB")
    draw = ImageDraw.Draw(masked)
    for asset in assets:
        draw.rectangle(asset.get("crop_bbox") or asset["bbox"], fill="white")
    bands = raw_bands(masked)
    substantial = [box for box in bands if box[3] - box[1] > 5]
    if not substantial:
        return []
    median_height = statistics.median(box[3] - box[1] for box in substantial)
    lines: list[list[int]] = []
    for box in bands:
        x0, y0, x1, y1 = box
        height = y1 - y0
        if height <= 5:
            candidates = [line for line in lines
                          if y0 - line[3] <= 6 and min(x1, line[2]) > max(x0, line[0])]
            if candidates:
                candidates[-1][0] = min(candidates[-1][0], x0)
                candidates[-1][2] = max(candidates[-1][2], x1)
                candidates[-1][3] = max(candidates[-1][3], y1)
            continue
        nearby = [asset for asset in assets
                  if y0 <= (asset.get("crop_bbox") or asset["bbox"])[3] + 120
                  and y1 >= (asset.get("crop_bbox") or asset["bbox"])[1]]
        if nearby:
            asset_boxes = [(asset.get("crop_bbox") or asset["bbox"]) for asset in nearby]
            chosen = max(asset_boxes, key=lambda item: (item[2] - item[0]) * (item[3] - item[1]))
            split_x = chosen[0] if (chosen[0] + chosen[2]) / 2 > image.width / 2 else chosen[2]
            split_lines = []
            for left, right in ((0, split_x), (split_x, image.width)):
                crop = masked.crop((left, y0, right, y1))
                split_lines.extend(item for item in raw_bands(crop, y0, left)
                                   if item[3] - item[1] > 5)
            if len(split_lines) > 1 or height > median_height * 1.7:
                lines.extend(split_lines)
                continue
        lines.append(box)
    ordered = sorted(lines, key=lambda box: (box[1], box[0]))
    merged: list[list[int]] = []
    for box in ordered:
        if merged:
            prior = merged[-1]
            vertical_overlap = min(prior[3], box[3]) - max(prior[1], box[1])
            minimum_height = min(prior[3] - prior[1], box[3] - box[1])
            horizontal_gap = box[0] - prior[2]
            if (box[0] >= prior[0] and vertical_overlap >= minimum_height * .7
                    and 0 <= horizontal_gap <= 20):
                prior[:] = [min(prior[0], box[0]), min(prior[1], box[1]),
                            max(prior[2], box[2]), max(prior[3], box[3])]
                continue
        merged.append(box)
    return merged


def run(image_path: Path, ocr_path: Path, output_path: Path, crop_dir: Path,
        base_url: str, model: str) -> dict[str, Any]:
    payload = json.loads(ocr_path.read_text(encoding="utf-8"))
    with Image.open(image_path) as source:
        image = source.convert("RGB")
    boxes = detect_lines(image, payload.get("assets", []))
    crop_dir.mkdir(parents=True, exist_ok=True)
    texts = []
    geometry = []
    for index, box in enumerate(boxes, 1):
        x0, y0, x1, y1 = box
        padded = [max(0, x0 - 3), max(0, y0 - 3), min(image.width, x1 + 3), min(image.height, y1 + 3)]
        crop = crop_dir / f"line-{index:03d}.png"
        image.crop(tuple(padded)).save(crop)
        text = dual_lmstudio_ocr.request_ocr(
            crop, base_url, model,
            "Transcribe only this single printed text line. Return plain text only. Do not explain.",
        ).strip().replace("\n", " ")
        if not text:
            continue
        texts.append(text)
        geometry.append({"id": f"L{len(texts):03d}", "bbox": box, "text": text,
                         "source": "ink-line-crop+paddleocr-vl"})
    payload["providers"]["text_witness"] = {"model": model, "text": "\n".join(texts),
                                               "mode": "independent-physical-line-crops"}
    payload["line_geometry"] = geometry
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
                           encoding="utf-8")
    return payload


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("ocr", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--crop-dir", required=True, type=Path)
    parser.add_argument("--base-url", default="http://127.0.0.1:1234/v1")
    parser.add_argument("--model", default="paddleocr-vl")
    args = parser.parse_args()
    try:
        result = run(args.image, args.ocr, args.output, args.crop_dir, args.base_url, args.model)
        print(json.dumps({"status": "success", "lines": len(result["line_geometry"]),
                          "output": str(args.output.resolve())}, ensure_ascii=False))
        return 0
    except Exception as error:
        print(f"line-crop-lmstudio-ocr: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
