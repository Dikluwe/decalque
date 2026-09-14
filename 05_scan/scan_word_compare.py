#!/usr/bin/env python3
"""Compare observed scan word geometry with candidate PDF glyph geometry."""

from __future__ import annotations

import argparse
import json
import math
import re
import sys
import unicodedata
from pathlib import Path
from typing import Any


NORMALIZE = re.compile(r"\s+", re.UNICODE)
TYPOGRAPHIC_MEASURES = ("x_height_pt", "ascender_height_pt", "descender_depth_pt")


def shape_distance(first: Any, second: Any) -> float | None:
    if not isinstance(first, dict) or not isinstance(second, dict):
        return None
    if first.get("version") != second.get("version") or first.get("bins") != second.get("bins"):
        return None
    fields = ("density", "centroid_x", "centroid_y")
    vector_fields = ("horizontal_projection", "vertical_projection")
    values: list[float] = []
    for field in fields:
        if not isinstance(first.get(field), (int, float)) or not isinstance(second.get(field), (int, float)):
            return None
        values.append(abs(first[field] - second[field]))
    for field in vector_fields:
        left, right = first.get(field), second.get(field)
        if not isinstance(left, list) or not isinstance(right, list) or len(left) != len(right):
            return None
        values.extend(abs(a - b) for a, b in zip(left, right))
    distances = [sum(values) / len(values)]
    left_occupancy, right_occupancy = first.get("occupancy"), second.get("occupancy")
    if (
        first.get("occupancy_rows") == second.get("occupancy_rows")
        and first.get("occupancy_columns") == second.get("occupancy_columns")
        and isinstance(left_occupancy, list)
        and isinstance(right_occupancy, list)
        and left_occupancy
        and len(left_occupancy) == len(right_occupancy)
    ):
        distances.append(
            sum(abs(a - b) for a, b in zip(left_occupancy, right_occupancy))
            / len(left_occupancy)
        )
    return max(distances)


def key(text: str | None) -> str:
    return NORMALIZE.sub("", text or "").casefold()


def logical_order(glyphs: list[dict[str, Any]]) -> list[dict[str, Any]]:
    ordered, start = [], 0
    while start < len(glyphs):
        first = glyphs[start]
        end = start + 1
        while end < len(glyphs) and glyphs[end]["font_ref"] == first["font_ref"] and glyphs[end]["position"][1] == first["position"][1]:
            end += 1
        run = glyphs[start:end]
        classes = {unicodedata.bidirectional(c) for glyph in run for c in (glyph.get("text") or "") if unicodedata.bidirectional(c) in {"L", "R", "AL"}}
        ordered.extend(reversed(run) if classes and classes <= {"R", "AL"} else run)
        start = end
    return ordered


def candidate_words(glyphs: list[dict[str, Any]]) -> list[dict[str, Any]]:
    words, current = [], []
    line_id, previous_y = 0, None

    def flush() -> None:
        nonlocal current
        if not current:
            return
        words.append(
            {
                "text": "".join(glyph.get("text") or "" for glyph in current),
                "line_id": line_id,
                "x0": min(glyph["position"][0] for glyph in current),
                "x1": max(glyph["position"][0] + glyph["advance"] for glyph in current),
                "baseline_y": sum(glyph["position"][1] for glyph in current) / len(current),
                "font_size_pt": max(glyph["font_size_pt"] for glyph in current),
            }
        )
        current = []

    for glyph in logical_order(glyphs):
        y = glyph["position"][1]
        if previous_y is not None and abs(y - previous_y) > 0.5:
            flush()
            line_id += 1
        text = glyph.get("text") or ""
        if text.isspace():
            flush()
        elif text:
            current.append(glyph)
        previous_y = y
    flush()
    return words


def compare(
    page: dict[str, Any], catalog: dict[str, Any], absolute_tolerance_pt: float = 1.5,
    relative_tolerance_em: float = 0.1, minimum_baseline_confidence: float = 0.5,
    typographic_absolute_tolerance_pt: float = 1.0,
    typographic_relative_tolerance_em: float = 0.06,
    typographic_shape_tolerance: float = 0.12,
) -> dict[str, Any]:
    candidates = candidate_words(catalog.get("glyphs", []))
    by_text: dict[str, list[dict[str, Any]]] = {}
    for word in candidates:
        by_text.setdefault(key(word["text"]), []).append(word)
    results, line_results = [], []
    for region in page.get("regions", []):
        for line in region.get("detected_lines", []):
            candidate_line_ids = set()
            for segment in line.get("word_segments", []):
                matches = by_text.get(key(segment.get("text")), [])
                if len(matches) != 1 or not segment.get("bbox_pt"):
                    results.append({"word": segment.get("text"), "status": "unknown"})
                    continue
                candidate = matches[0]
                candidate_line_ids.add(candidate["line_id"])
                scan = segment["bbox_pt"]
                tolerance = max(absolute_tolerance_pt, relative_tolerance_em * candidate["font_size_pt"])
                deltas = {
                    "start_x": scan[0] - candidate["x0"],
                    "end_x": scan[2] - candidate["x1"],
                    "width": (scan[2] - scan[0]) - (candidate["x1"] - candidate["x0"]),
                }
                horizontal_status = "preserved" if all(abs(value) <= tolerance for value in deltas.values()) else "violated"
                baseline = segment.get("baseline_y_pt")
                confidence = segment.get("baseline_confidence")
                if (
                    isinstance(baseline, (int, float))
                    and not isinstance(baseline, bool)
                    and math.isfinite(baseline)
                    and isinstance(confidence, (int, float))
                    and not isinstance(confidence, bool)
                    and math.isfinite(confidence)
                    and confidence >= minimum_baseline_confidence
                ):
                    deltas["baseline_y"] = baseline - candidate["baseline_y"]
                    vertical_status = "preserved" if abs(deltas["baseline_y"]) <= tolerance else "violated"
                else:
                    vertical_status = "unknown"
                if "violated" in (horizontal_status, vertical_status):
                    status = "violated"
                elif horizontal_status == vertical_status == "preserved":
                    status = "preserved"
                else:
                    status = "unknown"
                observed_profile = segment.get("typographic_profile") or {}
                candidate_profile = segment.get("candidate_typographic_profile") or {}
                typographic_deltas = {
                    measure: observed_profile[measure] - candidate_profile[measure]
                    for measure in TYPOGRAPHIC_MEASURES
                    if isinstance(observed_profile.get(measure), (int, float))
                    and isinstance(candidate_profile.get(measure), (int, float))
                }
                scale_y = (page.get("point_transform") or {}).get("scale_y_pt_per_px")
                quantization_tolerance = (
                    2 * scale_y
                    if isinstance(scale_y, (int, float))
                    and not isinstance(scale_y, bool)
                    and math.isfinite(scale_y)
                    and scale_y > 0
                    else 0.0
                )
                typographic_tolerance = max(
                    typographic_absolute_tolerance_pt,
                    typographic_relative_tolerance_em * candidate["font_size_pt"],
                    quantization_tolerance,
                )
                visual_distance = shape_distance(
                    observed_profile.get("ink_shape"), candidate_profile.get("ink_shape")
                )
                metric_violated = any(
                    abs(delta) > typographic_tolerance for delta in typographic_deltas.values()
                )
                shape_violated = (
                    visual_distance is not None and visual_distance > typographic_shape_tolerance
                )
                if typographic_deltas or visual_distance is not None:
                    typography_status = (
                        "violated" if metric_violated or shape_violated else "preserved"
                    )
                    typography_reason = None
                else:
                    typography_status = "unknown"
                    typography_reason = "no-comparable-typographic-measure"
                results.append({
                    "word": segment["text"], "status": status, "deltas_pt": deltas,
                    "tolerance_pt": tolerance, "scan_bbox_pt": scan,
                    "candidate_span_pt": [candidate["x0"], candidate["x1"]],
                    "candidate_baseline_y_pt": candidate["baseline_y"],
                    "scan_baseline_y_pt": baseline,
                    "baseline_confidence": confidence,
                    "minimum_baseline_confidence": minimum_baseline_confidence,
                    "candidate_line_id": candidate["line_id"],
                    "horizontal_status": horizontal_status,
                    "vertical_status": vertical_status,
                    "typography_status": typography_status,
                    "typography_reason": typography_reason,
                    "typographic_deltas_pt": typographic_deltas,
                    "typographic_tolerance_pt": typographic_tolerance,
                    "typographic_quantization_tolerance_pt": quantization_tolerance,
                    "typographic_shape_distance": visual_distance,
                    "typographic_shape_tolerance": typographic_shape_tolerance,
                    "observed_typographic_profile": observed_profile or None,
                    "candidate_typographic_profile": candidate_profile or None,
                })
            if len(candidate_line_ids) == 1:
                line_status = "preserved"
            elif len(candidate_line_ids) > 1:
                line_status = "violated"
            else:
                line_status = "unknown"
            line_results.append({"scan_line_id": line.get("id"), "status": line_status, "candidate_line_ids": sorted(candidate_line_ids)})
    counts = {status: sum(item["status"] == status for item in results) for status in ("preserved", "violated", "unknown")}
    return {
        "schema_version": 1, "words": results, "lines": line_results,
        "coverage": {"comparable": counts["preserved"] + counts["violated"], "total_scan": len(results)},
        "counts": counts,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("scan_json", type=Path)
    parser.add_argument("candidate_catalog", type=Path)
    parser.add_argument("--absolute-tolerance-pt", type=float, default=1.5)
    parser.add_argument("--relative-tolerance-em", type=float, default=0.1)
    parser.add_argument("--minimum-baseline-confidence", type=float, default=0.5)
    parser.add_argument("--typographic-absolute-tolerance-pt", type=float, default=1.0)
    parser.add_argument("--typographic-relative-tolerance-em", type=float, default=0.06)
    parser.add_argument("--typographic-shape-tolerance", type=float, default=0.12)
    args = parser.parse_args()
    try:
        pages = json.loads(args.scan_json.read_text(encoding="utf-8"))
        catalog = json.loads(args.candidate_catalog.read_text(encoding="utf-8"))
        if len(pages) != 1:
            raise ValueError("esta versão compara uma página por execução")
        json.dump(compare(
            pages[0], catalog, args.absolute_tolerance_pt, args.relative_tolerance_em,
            args.minimum_baseline_confidence, args.typographic_absolute_tolerance_pt,
            args.typographic_relative_tolerance_em, args.typographic_shape_tolerance,
        ), sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"scan-word-compare: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
