#!/usr/bin/env python3
"""Convert observed scan geometry from image pixels to candidate PDF points."""

from __future__ import annotations

import argparse
import copy
import json
import math
import sys
from pathlib import Path
from typing import Any


def positive_finite(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value > 0


def unknown_transform(reason: str, maximum_aspect_error: float) -> dict[str, Any]:
    return {
        "status": "unknown",
        "reason": reason,
        "scale_x_pt_per_px": None,
        "scale_y_pt_per_px": None,
        "aspect_relative_error": None,
        "maximum_aspect_error": maximum_aspect_error,
    }


def derive_transform(
    image_width: Any,
    image_height: Any,
    page_width_pt: Any,
    page_height_pt: Any,
    maximum_aspect_error: float = 0.005,
) -> dict[str, Any]:
    values = (image_width, image_height, page_width_pt, page_height_pt)
    if not all(positive_finite(value) for value in values):
        return unknown_transform("invalid-dimensions", maximum_aspect_error)
    image_aspect = image_width / image_height
    page_aspect = page_width_pt / page_height_pt
    aspect_error = abs(image_aspect / page_aspect - 1.0)
    if aspect_error > maximum_aspect_error:
        result = unknown_transform("aspect-ratio-mismatch", maximum_aspect_error)
        result["aspect_relative_error"] = aspect_error
        return result
    return {
        "status": "inferred",
        "reason": None,
        "scale_x_pt_per_px": page_width_pt / image_width,
        "scale_y_pt_per_px": page_height_pt / image_height,
        "aspect_relative_error": aspect_error,
        "maximum_aspect_error": maximum_aspect_error,
        "evidence": [{"method": "candidate-page-dimensions"}],
    }


def convert_bbox(bbox: Any, transform: dict[str, Any]) -> list[float] | None:
    if transform["status"] != "inferred" or not isinstance(bbox, list) or len(bbox) != 4:
        return None
    sx = transform["scale_x_pt_per_px"]
    sy = transform["scale_y_pt_per_px"]
    return [bbox[0] * sx, bbox[1] * sy, bbox[2] * sx, bbox[3] * sy]


def convert_polygon(polygon: Any, transform: dict[str, Any]) -> list[list[float]] | None:
    if transform["status"] != "inferred" or not isinstance(polygon, list):
        return None
    sx = transform["scale_x_pt_per_px"]
    sy = transform["scale_y_pt_per_px"]
    if any(not isinstance(point, list) or len(point) != 2 for point in polygon):
        return None
    return [[point[0] * sx, point[1] * sy] for point in polygon]


def convert_node(node: dict[str, Any], transform: dict[str, Any]) -> None:
    node["bbox_pt"] = convert_bbox(node.get("bbox"), transform)
    node["polygon_pt"] = convert_polygon(node.get("polygon"), transform)


def enrich_page(
    page: dict[str, Any], catalog: dict[str, Any], maximum_aspect_error: float = 0.005
) -> dict[str, Any]:
    output = copy.deepcopy(page)
    candidate_page = catalog.get("page", {})
    transform = derive_transform(
        output.get("width"),
        output.get("height"),
        candidate_page.get("width_pt"),
        candidate_page.get("height_pt"),
        maximum_aspect_error,
    )
    output["point_transform"] = transform
    for region in output.get("regions", []):
        convert_node(region, transform)
        for detected_line in region.get("detected_lines", []):
            convert_node(detected_line, transform)
            for word_segment in detected_line.get("word_segments", []):
                convert_node(word_segment, transform)
        for line in region.get("lines", []):
            convert_node(line, transform)
            for token in line.get("tokens", []):
                convert_node(token, transform)
    for line in output.get("unassigned_lines", []):
        convert_node(line, transform)
    return output


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("scan_json", type=Path)
    parser.add_argument("candidate_catalog", type=Path)
    parser.add_argument("--maximum-aspect-error", type=float, default=0.005)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        pages = json.loads(args.scan_json.read_text(encoding="utf-8"))
        catalog = json.loads(args.candidate_catalog.read_text(encoding="utf-8"))
        if len(pages) != 1:
            raise ValueError("o catálogo candidato descreve exatamente uma página")
        output = [enrich_page(pages[0], catalog, args.maximum_aspect_error)]
        json.dump(output, sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"coordinate-transform: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
