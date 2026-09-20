#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/frozen-document-typography.md
"""Discover, freeze and audit document-wide typography without per-line fitting."""

from __future__ import annotations

import hashlib
import json
import statistics
from typing import Any

import numpy as np
from PIL import Image


def _median(values: list[float]) -> float:
    return float(statistics.median(values)) if values else 0.0


def _row_bands(mask: np.ndarray, threshold: float, merge_gap: int = 0) -> list[tuple[int, int]]:
    projection = mask.sum(axis=1)
    active = projection > threshold
    bands, begin = [], None
    for y, value in enumerate(active):
        if value and begin is None:
            begin = y
        elif not value and begin is not None:
            bands.append((begin, y)); begin = None
    if begin is not None:
        bands.append((begin, mask.shape[0]))
    if not merge_gap or not bands:
        return bands
    merged = [bands[0]]
    for start, end in bands[1:]:
        if start - merged[-1][1] <= merge_gap:
            merged[-1] = (merged[-1][0], end)
        else:
            merged.append((start, end))
    return merged


def observe_geometry(image: Image.Image, page: int, width_pt: float,
                     height_pt: float) -> dict[str, Any]:
    gray = np.asarray(image.convert("L"), dtype=np.uint8)
    ink = gray < 180
    height, width = ink.shape
    body_top, body_bottom = round(height * 0.04), round(height * 0.94)
    body = ink[body_top:body_bottom]
    # Probe the lower body for the gutter: page-wide headings and illustrations commonly
    # span the upper third even when the running-text master is two-column.
    gutter_probe = ink[round(height * 0.45):round(height * 0.90)]
    vertical = gutter_probe.sum(axis=0)
    active = np.flatnonzero(vertical > max(2, body.shape[0] * 0.005))
    if not len(active):
        return {"page": page, "columns": [], "line_heights_px": [],
                "baseline_steps_px": [], "page_size_px": [width, height],
                "page_size_pt": [width_pt, height_pt]}
    left, right = int(active[0]), int(active[-1] + 1)
    center_lo, center_hi = round(width * 0.35), round(width * 0.65)
    low = vertical < max(2, gutter_probe.shape[0] * 0.04)
    runs, start = [], None
    for x in range(center_lo, center_hi):
        if low[x] and start is None:
            start = x
        elif not low[x] and start is not None:
            runs.append((start, x)); start = None
    if start is not None:
        runs.append((start, center_hi))
    gutter = max(runs, key=lambda run: run[1] - run[0], default=None)
    spanning_zones: list[dict[str, Any]] = []
    if gutter and gutter[1] - gutter[0] >= width * 0.025:
        g0, g1 = gutter
        window = max(60, round(height * 0.10))
        column_top = body_top
        for candidate in range(body_top, round(height * 0.65), max(2, round(height * 0.005))):
            end = min(body_bottom, candidate + window)
            gutter_density = float(ink[candidate:end, g0:g1].mean())
            left_rows = (ink[candidate:end, left:g0].sum(axis=1) > (g0 - left) * 0.015).sum()
            right_rows = (ink[candidate:end, g1:right].sum(axis=1) > (right - g1) * 0.015).sum()
            if gutter_density < 0.025 and min(left_rows, right_rows) >= window * 0.18:
                column_top = candidate
                break
        columns = [[left, column_top, g0, body_bottom],
                   [g1, column_top, right, body_bottom]]
        if column_top > body_top + height * 0.04:
            upper = ink[body_top:column_top, left:right]
            for start, end in _row_bands(upper, (right - left) * 0.012,
                                         merge_gap=max(3, round(height * 0.008))):
                y0, y1 = start + body_top, end + body_top
                band = ink[y0:y1, left:right]
                xs = np.flatnonzero(band.sum(axis=0) > 0)
                if not len(xs):
                    continue
                x0, x1 = left + int(xs[0]), left + int(xs[-1] + 1)
                zone_height = y1 - y0
                if zone_height >= height * 0.08:
                    kind = "visual"
                elif zone_height <= max(3, height * 0.008):
                    kind = "rule"
                else:
                    kind = "heading"
                spanning_zones.append({"kind": kind, "bbox": [x0, y0, x1, y1]})
    else:
        columns = [[left, body_top, right, body_bottom]]
    line_heights, baselines = [], []
    for x0, y0, x1, y1 in columns:
        projection = ink[y0:y1, x0:x1].sum(axis=1)
        row_active = projection > max(2, (x1 - x0) * 0.015)
        bands, begin = [], None
        for offset, value in enumerate(row_active):
            if value and begin is None:
                begin = offset
            elif not value and begin is not None:
                bands.append((begin + y0, offset + y0)); begin = None
        if begin is not None:
            bands.append((begin + y0, y1))
        text_bands = [(a, b) for a, b in bands if height * 0.006 <= b - a <= height * 0.035]
        line_heights.extend(b - a for a, b in text_bands)
        centers = [(a + b) / 2 for a, b in text_bands]
        baselines.extend(b - a for a, b in zip(centers, centers[1:])
                         if height * 0.008 <= b - a <= height * 0.05)
    return {"page": page, "columns": columns, "spanning_zones": spanning_zones,
            "line_heights_px": line_heights,
            "baseline_steps_px": baselines, "page_size_px": [width, height],
            "page_size_pt": [width_pt, height_pt]}


def freeze_profile(observations: list[dict[str, Any]], family: str) -> dict[str, Any]:
    usable = [item for item in observations if item.get("columns")]
    if not usable:
        raise ValueError("nenhuma observação geométrica utilizável")
    counts = [len(item["columns"]) for item in usable]
    column_count = int(statistics.mode(counts))
    heights, steps = [], []
    for item in usable:
        sy = float(item["page_size_pt"][1]) / float(item["page_size_px"][1])
        heights.extend(float(value) * sy for value in item.get("line_heights_px", []))
        steps.extend(float(value) * sy for value in item.get("baseline_steps_px", []))
    font_size = _median(heights)
    leading = _median(steps)
    profile = {"schema_version": 1, "state": "frozen",
               "source_pages": [item["page"] for item in usable],
               "page_master": {"column_count": column_count,
                               "column_policy": "document-general"},
               "classes": {"body": {"family": family,
                                     "font_size_pt": round(font_size, 3),
                                     "baseline_step_pt": round(leading, 3),
                                     "horizontal_scale": 1.0,
                                     "tracking_em": 0.0}},
               "policy": {"loop_phase": "closed", "per_line_refit": False,
                          "differences_create_marks": True}}
    canonical = json.dumps(profile, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    profile["profile_hash"] = hashlib.sha256(canonical.encode()).hexdigest()
    return profile


def audit_observation(profile: dict[str, Any], observation: dict[str, Any],
                      tolerance: float = 0.15) -> dict[str, Any]:
    if profile.get("state") != "frozen":
        raise ValueError("perfil deve estar congelado")
    expected = profile["classes"]["body"]
    sy = observation["page_size_pt"][1] / observation["page_size_px"][1]
    observed_size = _median(observation.get("line_heights_px", [])) * sy
    observed_step = _median(observation.get("baseline_steps_px", [])) * sy
    marks = []
    if len(observation.get("columns", [])) != profile["page_master"]["column_count"]:
        marks.append({"kind": "column-count", "observed": len(observation.get("columns", []))})
    for kind, observed, target in (("font-size", observed_size, expected["font_size_pt"]),
                                   ("baseline-step", observed_step, expected["baseline_step_pt"])):
        if target and abs(observed - target) / target > tolerance:
            marks.append({"kind": kind, "observed_pt": round(observed, 3),
                          "profile_pt": target})
    return {"page": observation["page"], "profile_hash": profile["profile_hash"],
            "status": "review" if marks else "conformant", "marks": marks}
