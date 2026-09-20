#!/usr/bin/env python3
"""Infer mirrored left/right book-page masters from observed page geometry."""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from pathlib import Path
from typing import Any


def bbox(value: Any, name: str) -> list[float]:
    if not isinstance(value, list) or len(value) != 4:
        raise ValueError(f"{name} deve conter quatro números")
    result = [float(item) for item in value]
    if result[2] <= result[0] or result[3] <= result[1]:
        raise ValueError(f"{name} degenerada")
    return result


def side_for(page: dict[str, Any], index: int, width_px: float, first_side: str) -> tuple[str, str]:
    pagination = page.get("pagination")
    if pagination:
        box = bbox(pagination.get("bbox_px"), "pagination.bbox_px")
        center = (box[0] + box[2]) / (2 * width_px)
        if center <= 0.40:
            return "left", "pagination-position"
        if center >= 0.60:
            return "right", "pagination-position"
    printed = page.get("printed_page")
    if isinstance(printed, int) and printed > 0:
        return ("left" if printed % 2 == 0 else "right"), "printed-page-parity"
    if (index % 2 == 0) == (first_side == "left"):
        return "left", "physical-sequence"
    return "right", "physical-sequence"


def median(values: list[float]) -> float | None:
    return round(statistics.median(values), 4) if values else None


def infer(source: dict[str, Any], first_side: str) -> dict[str, Any]:
    if source.get("schema_version") != 1:
        raise ValueError("schema_version não suportada")
    page = source.get("page", {})
    width_pt, height_pt = float(page.get("width_pt", 0)), float(page.get("height_pt", 0))
    width_px, height_px = float(page.get("source_width_px", 0)), float(page.get("source_height_px", 0))
    if min(width_pt, height_pt, width_px, height_px) <= 0:
        raise ValueError("dimensões da página devem ser positivas")
    pages = source.get("pages")
    if not isinstance(pages, list) or not pages:
        raise ValueError("pages deve ser uma lista não vazia")
    sx, sy = width_pt / width_px, height_pt / height_px
    classified = []
    measurements: dict[str, dict[str, list[float]]] = {
        side: {key: [] for key in ("inner", "outer", "top", "bottom", "body_width",
                                    "first_line_indent", "leading", "pagination_x", "pagination_y")}
        for side in ("left", "right")
    }
    for index, observed in enumerate(pages):
        side, evidence = side_for(observed, index, width_px, first_side)
        result = {"pdf_page": observed.get("pdf_page"), "printed_page": observed.get("printed_page"),
                  "side": side, "classification_evidence": evidence}
        values = measurements[side]
        if observed.get("body_bbox_px"):
            body = bbox(observed["body_bbox_px"], "body_bbox_px")
            left, right = body[0] * sx, width_pt - body[2] * sx
            values["inner"].append(right if side == "left" else left)
            values["outer"].append(left if side == "left" else right)
            values["top"].append(body[1] * sy)
            values["bottom"].append(height_pt - body[3] * sy)
            values["body_width"].append((body[2] - body[0]) * sx)
        if observed.get("first_line_indent_px") is not None:
            values["first_line_indent"].append(float(observed["first_line_indent_px"]) * sx)
        baselines = observed.get("line_baselines_px", [])
        values["leading"].extend((b - a) * sy for a, b in zip(baselines, baselines[1:]) if b > a)
        if observed.get("pagination"):
            footer = bbox(observed["pagination"]["bbox_px"], "pagination.bbox_px")
            values["pagination_x"].append(((footer[0] + footer[2]) / 2) * sx)
            values["pagination_y"].append(((footer[1] + footer[3]) / 2) * sy)
        classified.append(result)
    masters = {}
    for side, values in measurements.items():
        masters[side] = {"sample_count": sum(item["side"] == side for item in classified),
                         "inner_margin_pt": median(values["inner"]),
                         "outer_margin_pt": median(values["outer"]),
                         "top_margin_pt": median(values["top"]),
                         "bottom_margin_pt": median(values["bottom"]),
                         "body_width_pt": median(values["body_width"]),
                         "first_line_indent_pt": median(values["first_line_indent"]),
                         "leading_pt": median(values["leading"]),
                         "pagination_center_pt": [median(values["pagination_x"]),
                                                   median(values["pagination_y"])]}
    return {"schema_version": 1, "status": "observed", "page": {
                "width_pt": width_pt, "height_pt": height_pt,
                "source_width_px": width_px, "source_height_px": height_px},
            "masters": masters, "classified_pages": classified,
            "policy": {"position_precedes_printed_parity": True,
                       "printed_parity_precedes_physical_sequence": True,
                       "first_physical_side": first_side}}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("observations", type=Path)
    parser.add_argument("--first-page-side", choices=("left", "right"), default="right")
    args = parser.parse_args()
    try:
        result = infer(json.loads(args.observations.read_text()), args.first_page_side)
        json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"book-page-masters: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
