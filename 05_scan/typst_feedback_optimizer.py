#!/usr/bin/env python3
"""Close the scan-to-Typst loop using horizontal and vertical ink projections."""

from __future__ import annotations

import argparse
import copy
import json
import shutil
import sys
from pathlib import Path

import numpy as np
from PIL import Image

import typst_reconstruction_pipeline


def ink_projections(path: Path) -> tuple[np.ndarray, np.ndarray, tuple[int, int]]:
    with Image.open(path) as source:
        pixels = np.asarray(source.convert("L"), dtype=np.float64)
    ink = 1.0 - pixels / 255.0
    return ink.mean(axis=1), ink.mean(axis=0), (pixels.shape[1], pixels.shape[0])


def ink_zones(path: Path, merge_gap: int, minimum_height: int = 5) -> list[dict]:
    with Image.open(path) as source:
        ink = 1.0 - np.asarray(source.convert("L"), dtype=np.float64) / 255.0
    active = ink.mean(axis=1) > 0.001
    runs = []
    start = None
    for index, present in enumerate(active):
        if present and start is None:
            start = index
        if start is not None and (not present or index == len(active) - 1):
            runs.append([start, index if not present else index + 1])
            start = None
    merged = []
    for run in runs:
        if merged and run[0] - merged[-1][1] <= merge_gap:
            merged[-1][1] = run[1]
        else:
            merged.append(run)
    zones = []
    for top, bottom in merged:
        if bottom - top < minimum_height:
            continue
        weights = ink[top:bottom].sum(axis=0)
        if weights.sum() == 0:
            continue
        cumulative = np.cumsum(weights) / weights.sum()
        left = int(np.searchsorted(cumulative, 0.01))
        right = int(np.searchsorted(cumulative, 0.99) + 1)
        center_x = float(np.average(np.arange(len(weights)), weights=weights))
        zones.append({"top": top, "bottom": bottom, "center_y": (top + bottom) / 2,
                      "left": left, "right": right, "center_x": center_x,
                      "height": bottom - top})
    return zones


def compare_zones(reference: list[dict], candidate: list[dict]) -> dict:
    if len(reference) != len(candidate):
        return {"status": "unknown", "reference_count": len(reference),
                "candidate_count": len(candidate), "pairs": [], "gaps": []}
    pairs = []
    for index, (left, right) in enumerate(zip(reference, candidate)):
        pairs.append({"zone": index, "reference": left, "candidate": right,
                      "delta_x_px": round(left["center_x"] - right["center_x"], 3),
                      "delta_y_px": round(left["center_y"] - right["center_y"], 3),
                      "height_ratio": round(left["height"] / right["height"], 4),
                      "width_ratio": round((left["right"] - left["left"]) /
                                           (right["right"] - right["left"]), 4)})
    gaps = []
    for index in range(len(reference) - 1):
        reference_gap = reference[index + 1]["top"] - reference[index]["bottom"]
        candidate_gap = candidate[index + 1]["top"] - candidate[index]["bottom"]
        gaps.append({"after_zone": index, "reference_gap_px": reference_gap,
                     "candidate_gap_px": candidate_gap,
                     "gap_delta_px": reference_gap - candidate_gap})
    return {"status": "comparable", "reference_count": len(reference),
            "candidate_count": len(candidate), "pairs": pairs, "gaps": gaps}


def shifted(values: np.ndarray, amount: int) -> np.ndarray:
    result = np.zeros_like(values)
    if amount > 0:
        result[amount:] = values[:-amount]
    elif amount < 0:
        result[:amount] = values[-amount:]
    else:
        result[:] = values
    return result


def best_shift(reference: np.ndarray, candidate: np.ndarray, maximum: int) -> int:
    return min(range(-maximum, maximum + 1), key=lambda amount: np.abs(reference - shifted(candidate, amount)).mean())


def translate_manifest(manifest: dict, shift_x: float, shift_y: float) -> dict:
    translated = copy.deepcopy(manifest)
    for region in translated["regions"]:
        x0, y0, x1, y1 = region["bbox_px"]
        region["bbox_px"] = [x0 + shift_x, y0 + shift_y, x1 + shift_x, y1 + shift_y]
    return translated


def translate_manifest_zones(manifest: dict, pairs: list[dict], raster_size: tuple[int, int]) -> dict:
    translated = copy.deepcopy(manifest)
    page = translated["page"]
    scale_x = float(page["source_width_px"]) / raster_size[0]
    scale_y = float(page["source_height_px"]) / raster_size[1]
    for region, pair in zip(translated["regions"], pairs):
        shift_x = pair["delta_x_px"] * scale_x
        shift_y = pair["delta_y_px"] * scale_y
        x0, y0, x1, y1 = region["bbox_px"]
        region["bbox_px"] = [x0 + shift_x, y0 + shift_y, x1 + shift_x, y1 + shift_y]
        height_ratio = pair["height_ratio"] ** 0.5
        height_ratio = min(1.07, max(0.93, height_ratio))
        if region.get("kind") == "text":
            font = region["font"]
            font["size_pt"] = float(font["size_pt"]) * height_ratio
            if "leading_pt" in font:
                font["leading_pt"] = float(font["leading_pt"]) * height_ratio
            center_x = (region["bbox_px"][0] + region["bbox_px"][2]) / 2
            half_width = (region["bbox_px"][2] - region["bbox_px"][0]) * height_ratio / 2
            region["bbox_px"][0], region["bbox_px"][2] = center_x - half_width, center_x + half_width
        elif region.get("kind") == "image":
            center_x, center_y = (region["bbox_px"][0] + region["bbox_px"][2]) / 2, (region["bbox_px"][1] + region["bbox_px"][3]) / 2
            half_width = (region["bbox_px"][2] - region["bbox_px"][0]) * height_ratio / 2
            half_height = (region["bbox_px"][3] - region["bbox_px"][1]) * height_ratio / 2
            region["bbox_px"] = [center_x - half_width, center_y - half_height,
                                 center_x + half_width, center_y + half_height]
    return translated


def optimize(manifest_path: Path, source_pdf: Path, output_dir: Path, source_page: int, dpi: int,
             horizontal_threshold: float, vertical_threshold: float,
             max_cycles: int, max_shift_px: int, feedback_mode: str = "zones",
             zone_position_threshold: float = 2.0, zone_gap_threshold: float = 2.0,
             zone_merge_gap: int = 12) -> dict:
    if horizontal_threshold < 0 or vertical_threshold < 0:
        raise ValueError("limites de erro não podem ser negativos")
    if max_cycles < 1 or max_shift_px < 0 or dpi < 1 or source_page < 1:
        raise ValueError("ciclos, DPI e página devem ser positivos; deslocamento não pode ser negativo")
    current = typst_reconstruction_pipeline.typst_page_materializer.load_manifest(manifest_path)
    for region in current["regions"]:
        if region.get("kind") == "image":
            region["path"] = str((manifest_path.parent / region["path"]).resolve())
    history = []
    no_improvement = 0
    previous_errors = None
    previous_objective = None
    final_run = None
    final_rendered_manifest = None
    best = None
    status = "not_converged"
    reason = "max_cycles"
    for cycle in range(max_cycles):
        cycle_dir = output_dir / f"cycle-{cycle:03d}"
        cycle_manifest = cycle_dir / "manifest.json"
        cycle_dir.mkdir(parents=True, exist_ok=True)
        cycle_manifest.write_text(json.dumps(current, ensure_ascii=False, indent=2))
        run = typst_reconstruction_pipeline.run(cycle_manifest, source_pdf, cycle_dir, source_page, dpi)
        final_run = run
        final_rendered_manifest = copy.deepcopy(current)
        metrics = run["visual_comparison"]
        if metrics["status"] != "comparable":
            reason = "not_comparable"
            break
        errors = (metrics["horizontal_line_error"], metrics["vertical_line_error"])
        entry = {"cycle": cycle, "horizontal_line_error": errors[0],
                 "vertical_line_error": errors[1], "shift_x_px": 0, "shift_y_px": 0,
                 "manifest": str(cycle_manifest.resolve()), "pdf": run["pdf"]}
        history.append(entry)
        zone_report = compare_zones(ink_zones(Path(run["reference_png"]), zone_merge_gap),
                                    ink_zones(Path(run["candidate_png"]), zone_merge_gap))
        entry["zones"] = zone_report
        position_penalty = max((abs(pair["delta_y_px"]) for pair in zone_report["pairs"]), default=1000)
        gap_penalty = max((abs(gap["gap_delta_px"]) for gap in zone_report["gaps"]), default=1000)
        objective = position_penalty + gap_penalty + 100 * (errors[0] + errors[1])
        entry["objective"] = round(objective, 6)
        if best is None or objective < best[0]:
            best = (objective, cycle, copy.deepcopy(current), run)
        zone_positions_ok = zone_report["status"] == "comparable" and all(
            abs(pair[axis]) <= zone_position_threshold
            for pair in zone_report["pairs"] for axis in ("delta_x_px", "delta_y_px"))
        zone_gaps_ok = zone_report["status"] == "comparable" and all(
            abs(gap["gap_delta_px"]) <= zone_gap_threshold for gap in zone_report["gaps"])
        zones_ok = feedback_mode == "global" or (zone_positions_ok and zone_gaps_ok)
        if errors[0] <= horizontal_threshold and errors[1] <= vertical_threshold and zones_ok:
            status, reason = "converged", "thresholds_met"
            break
        horizontal_ref, vertical_ref, raster_size = ink_projections(Path(run["reference_png"]))
        horizontal_candidate, vertical_candidate, _ = ink_projections(Path(run["candidate_png"]))
        if feedback_mode == "zones":
            if zone_report["status"] != "comparable" or len(zone_report["pairs"]) != len(current["regions"]):
                reason = "zones_unknown"
                break
            current = translate_manifest_zones(current, zone_report["pairs"], raster_size)
            shift_x = max((abs(pair["delta_x_px"]) for pair in zone_report["pairs"]), default=0)
            shift_y = max((abs(pair["delta_y_px"]) for pair in zone_report["pairs"]), default=0)
        else:
            shift_y = best_shift(horizontal_ref, horizontal_candidate, max_shift_px)
            shift_x = best_shift(vertical_ref, vertical_candidate, max_shift_px)
        entry["shift_x_px"], entry["shift_y_px"] = shift_x, shift_y
        if shift_x == 0 and shift_y == 0:
            reason = "zero_adjustment"
            break
        stalled = (previous_objective is not None and objective >= previous_objective) if feedback_mode == "zones" else (
            previous_errors is not None and errors[0] >= previous_errors[0] and errors[1] >= previous_errors[1])
        if stalled:
            no_improvement += 1
        else:
            no_improvement = 0
        if no_improvement >= 2:
            reason = "stagnation"
            break
        if feedback_mode == "global":
            page = current["page"]
            manifest_shift_x = shift_x * float(page["source_width_px"]) / raster_size[0]
            manifest_shift_y = shift_y * float(page["source_height_px"]) / raster_size[1]
            current = translate_manifest(current, manifest_shift_x, manifest_shift_y)
        previous_errors = errors
        previous_objective = objective
    if final_run is None:
        raise RuntimeError("nenhum ciclo foi executado")
    if status != "converged" and best is not None:
        _, selected_cycle, final_rendered_manifest, final_run = best
    else:
        selected_cycle = history[-1]["cycle"]
    output_dir.mkdir(parents=True, exist_ok=True)
    final_manifest = output_dir / "optimized-manifest.json"
    final_manifest.write_text(json.dumps(final_rendered_manifest, ensure_ascii=False, indent=2))
    final_pdf = output_dir / "reconstruction.pdf"
    shutil.copy2(final_run["pdf"], final_pdf)
    for name, source_key in [("candidate.png", "candidate_png"), ("reference.png", "reference_png"),
                             ("difference.png", "difference_png")]:
        shutil.copy2(final_run[source_key], output_dir / name)
    return {"schema_version": 1, "status": status, "reason": reason,
            "thresholds": {"horizontal_line_error": horizontal_threshold,
                           "vertical_line_error": vertical_threshold,
                           "zone_position_px": zone_position_threshold,
                           "zone_gap_px": zone_gap_threshold},
            "feedback_mode": feedback_mode,
            "cycles_executed": len(history), "history": history,
            "selected_cycle": selected_cycle,
            "optimized_manifest": str(final_manifest.resolve()), "pdf": str(final_pdf.resolve()),
            "final_metrics": final_run["visual_comparison"]}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("source_pdf", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--source-page", type=int, default=1)
    parser.add_argument("--dpi", type=int, default=180)
    parser.add_argument("--horizontal-threshold", type=float, default=0.002)
    parser.add_argument("--vertical-threshold", type=float, default=0.002)
    parser.add_argument("--max-cycles", type=int, default=8)
    parser.add_argument("--max-shift-px", type=int, default=40)
    parser.add_argument("--feedback-mode", choices=("zones", "global"), default="zones")
    parser.add_argument("--zone-position-threshold", type=float, default=2.0)
    parser.add_argument("--zone-gap-threshold", type=float, default=2.0)
    parser.add_argument("--zone-merge-gap", type=int, default=12)
    args = parser.parse_args()
    try:
        result = optimize(args.manifest, args.source_pdf, args.output_dir, args.source_page, args.dpi,
                          args.horizontal_threshold, args.vertical_threshold,
                          args.max_cycles, args.max_shift_px, args.feedback_mode,
                          args.zone_position_threshold, args.zone_gap_threshold,
                          args.zone_merge_gap)
        json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"typst-feedback-optimizer: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
