#!/usr/bin/env python3
"""Compare two LM Studio vision OCR models and capture Ovis image regions."""

from __future__ import annotations

import argparse
import base64
import difflib
import json
import mimetypes
import re
import sys
import urllib.request
from pathlib import Path
from typing import Any


OVIS_IMAGE = re.compile(
    r'<img\s+src=["\']images/bbox_(\d+)_(\d+)_(\d+)_(\d+)\.[^"\']+["\']\s*/?>',
    re.IGNORECASE,
)


def image_data_url(path: Path) -> str:
    mime = mimetypes.guess_type(path.name)[0] or "image/png"
    return f"data:{mime};base64,{base64.b64encode(path.read_bytes()).decode('ascii')}"


def request_ocr(path: Path, base_url: str, model: str, prompt: str) -> str:
    payload = {
        "model": model,
        "temperature": 0,
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "text", "text": prompt},
                    {"type": "image_url", "image_url": {"url": image_data_url(path)}},
                ],
            }
        ],
    }
    request = urllib.request.Request(
        f"{base_url.rstrip('/')}/chat/completions",
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=300) as response:
        result = json.load(response)
    return result["choices"][0]["message"]["content"]


def normalized_text(value: str) -> str:
    value = OVIS_IMAGE.sub(" ", value)
    value = re.sub(r"<[^>]+>", " ", value)
    value = re.sub(r"^[#>*+-]+\s*", "", value, flags=re.MULTILINE)
    value = re.sub(r"\.{3,}", " ", value)
    return " ".join(value.casefold().split())


def padded_bbox(bbox: list[int], width: int, height: int, padding_ratio: float) -> list[int]:
    padding_x = round(width * padding_ratio)
    padding_top = round(height * padding_ratio)
    padding_bottom = round(height * padding_ratio / 3)
    return [max(0, bbox[0] - padding_x), max(0, bbox[1] - padding_top),
            min(width, bbox[2] + padding_x), min(height, bbox[3] + padding_bottom)]


def capture_regions(
    image_path: Path, ovis_text: str, asset_dir: Path, padding_ratio: float = 0.015
) -> list[dict[str, Any]]:
    from PIL import Image

    with Image.open(image_path) as image:
        width, height = image.size
        assets = []
        asset_dir.mkdir(parents=True, exist_ok=True)
        for index, match in enumerate(OVIS_IMAGE.finditer(ovis_text), 1):
            normalized_bbox = [int(value) for value in match.groups()]
            x0, y0, x1, y1 = normalized_bbox
            bbox = [
                round(x0 * width / 1000),
                round(y0 * height / 1000),
                round(x1 * width / 1000),
                round(y1 * height / 1000),
            ]
            source_bbox = bbox
            crop_bbox = padded_bbox(source_bbox, width, height, padding_ratio)
            if crop_bbox[2] <= crop_bbox[0] or crop_bbox[3] <= crop_bbox[1]:
                continue
            output = asset_dir / f"region-{index:03d}.png"
            image.crop(tuple(crop_bbox)).save(output)
            assets.append(
                {
                    "source": "ovisocr2",
                    "normalized_bbox": normalized_bbox,
                    "bbox": source_bbox,
                    "crop_bbox": crop_bbox,
                    "crop_padding_ratio": padding_ratio,
                    "coordinate_space": "image-pixels-ydown",
                    "path": str(output.resolve()),
                }
            )
    return assets


def compare_responses(ovis_text: str, paddle_text: str) -> dict[str, Any]:
    left = normalized_text(ovis_text)
    right = normalized_text(paddle_text)
    ratio = difflib.SequenceMatcher(None, left, right, autojunk=False).ratio()
    return {
        "status": "agreement" if ratio >= 0.9 else "difference",
        "similarity": round(ratio, 4),
        "ovis_normalized_text": left,
        "paddle_normalized_text": right,
    }


def run(
    image_path: Path,
    asset_dir: Path,
    base_url: str,
    ovis_model: str,
    paddle_model: str,
    crop_padding: float = 0.015,
) -> dict[str, Any]:
    ovis_text = request_ocr(
        image_path,
        base_url,
        ovis_model,
        "Transcribe this page preserving layout as Markdown. Emit image regions with their bbox tags.",
    )
    paddle_text = request_ocr(
        image_path,
        base_url,
        paddle_model,
        "Transcribe every visible word on this page. Preserve line and paragraph breaks. Do not invent text.",
    )
    return {
        "schema_version": 1,
        "input": str(image_path.resolve()),
        "providers": {
            "layout": {"model": ovis_model, "text": ovis_text},
            "text_witness": {"model": paddle_model, "text": paddle_text},
        },
        "comparison": compare_responses(ovis_text, paddle_text),
        "assets": capture_regions(image_path, ovis_text, asset_dir, crop_padding),
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("--asset-dir", type=Path, required=True)
    parser.add_argument("--base-url", default="http://127.0.0.1:1234/v1")
    parser.add_argument("--ovis-model", default="ovisocr2")
    parser.add_argument("--paddle-model", default="paddleocr-vl")
    parser.add_argument("--crop-padding", type=float, default=0.015)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        json.dump(
            run(args.image, args.asset_dir, args.base_url, args.ovis_model, args.paddle_model, args.crop_padding),
            sys.stdout,
            ensure_ascii=False,
            indent=2,
        )
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"dual-lmstudio-ocr: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
