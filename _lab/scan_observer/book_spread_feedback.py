#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/book-spread-feedback.md
# @updated 2026-09-15
"""Optimize real book-spread typography with deterministic coordinate feedback."""

from __future__ import annotations

import argparse
import copy
import json
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image

import book_spread_example
import line_diff_analyzer
from typst_feedback_optimizer import ink_zones
from typst_reconstruction_pipeline import compare_images


def command(args: list[str]) -> None:
    process = subprocess.run(args, capture_output=True, text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or "comando externo falhou")


def source_for(ocr: dict, profile: dict, pages: list[int], family: str,
               parameters: dict[str, object], title_family: str | None = None) -> str:
    width, height = profile["page"]["width_pt"], profile["page"]["height_pt"]
    left = copy.deepcopy(profile["masters"]["left"])
    right = copy.deepcopy(profile["masters"]["right"])
    for master in (left, right):
        if master.get("sample_count") or master is left:
            master["top_margin_pt"] = parameters["top_pt"]
            master["body_width_pt"] = parameters["body_width_pt"]
            master["first_line_indent_pt"] = parameters["indent_pt"]
            master["leading_pt"] = parameters["leading_pt"]
            master["paragraph_spacing_pt"] = parameters["paragraph_spacing_pt"]
    if not right.get("sample_count"):
        right = book_spread_example.mirrored(left, width)
    by_number = {page["page_number"]: page for page in ocr["pages"]}
    rendered = []
    for number in pages:
        page = by_number[number]
        printed = int(page["text"].splitlines()[-1])
        side = "left" if printed % 2 == 0 else "right"
        rendered.append(book_spread_example.render_page(
            page, left if side == "left" else right, side, width, height, family,
            float(parameters["size_pt"]), float(parameters["title_size_pt"]),
            title_family, str(parameters["title_weights"].get(str(number), "regular"))))
    return "\n#pagebreak()\n".join(rendered) + "\n"


def evaluate(root: Path, source_pdf: Path, source_pages: list[int], dpi: int,
             ocr: dict, profile: dict, family: str, font_path: Path,
             parameters: dict[str, object], label: str, title_family: str | None = None) -> dict:
    trial = root / label
    trial.mkdir(parents=True, exist_ok=True)
    typ, pdf = trial / "reconstruction.typ", trial / "reconstruction.pdf"
    typ.write_text(source_for(ocr, profile, source_pages, family, parameters, title_family))
    command(["typst", "compile", "--font-path", str(font_path), "--root", "/", str(typ), str(pdf)])
    command(["pdftoppm", "-r", str(dpi), "-gray", "-png", str(pdf), str(trial / "candidate")])
    metrics = []
    for index, page in enumerate(source_pages, 1):
        reference = root / f"reference-{page:03d}.png"
        if not reference.exists():
            command(["pdftoppm", "-f", str(page), "-l", str(page), "-r", str(dpi),
                     "-gray", "-png", "-singlefile", str(source_pdf), str(reference.with_suffix(""))])
        candidate = trial / f"candidate-{index}.png"
        item = compare_images(reference, candidate, trial / f"difference-{index}.png")
        reference_zones = ink_zones(reference, 0, 3)
        candidate_zones = ink_zones(candidate, 0, 3)
        # Ignore isolated scan border/ornament/footer bands; paragraph lines remain in the body.
        raster_height = item["reference_size"][1]
        reference_lines = [z for z in reference_zones if 0.08 * raster_height < z["center_y"] < 0.92 * raster_height]
        candidate_lines = [z for z in candidate_zones if 0.08 * raster_height < z["center_y"] < 0.92 * raster_height]
        item["body_zone_count"] = len(candidate_lines)
        item["reference_body_zone_count"] = len(reference_lines)
        item["zone_count_error"] = abs(len(reference_lines) - len(candidate_lines)) / max(1, len(reference_lines))
        reference_line_height = float(np.median([z["height"] for z in reference_lines])) if reference_lines else 0.0
        candidate_line_height = float(np.median([z["height"] for z in candidate_lines])) if candidate_lines else 0.0
        item["reference_body_line_height_px"] = reference_line_height
        item["body_line_height_px"] = candidate_line_height
        item["body_font_size_error"] = abs(reference_line_height - candidate_line_height) / max(1.0, reference_line_height)
        reference_gaps = [b["top"] - a["bottom"] for a, b in zip(reference_lines, reference_lines[1:])]
        candidate_gaps = [b["top"] - a["bottom"] for a, b in zip(candidate_lines, candidate_lines[1:])]
        reference_p90 = float(np.quantile(reference_gaps, 0.9)) if reference_gaps else 0.0
        candidate_p90 = float(np.quantile(candidate_gaps, 0.9)) if candidate_gaps else 0.0
        item["body_gap_p90_px"] = candidate_p90
        item["reference_body_gap_p90_px"] = reference_p90
        item["gap_quantile_error"] = abs(reference_p90 - candidate_p90) / max(1.0, reference_p90)
        reference_median = float(np.median(reference_gaps)) if reference_gaps else 0.0
        reference_high = float(np.quantile(reference_gaps, 0.975)) if reference_gaps else 0.0
        candidate_high = float(np.quantile(candidate_gaps, 0.975)) if candidate_gaps else 0.0
        uniform_reference = bool(reference_gaps and reference_high <= 1.5 * max(1.0, reference_median))
        item["paragraph_gap_status"] = "comparable" if uniform_reference else "unknown"
        item["body_gap_p975_px"] = candidate_high
        item["reference_body_gap_p975_px"] = reference_high
        item["paragraph_gap_error"] = (
            abs(reference_high - candidate_high) / max(1.0, reference_high)
            if uniform_reference else 0.0)
        scale_y = dpi / 72.0
        expected_title_y = (float(parameters["top_pt"]) - float(parameters["leading_pt"]) / 2) * scale_y
        reference_title = min(reference_zones, key=lambda z: abs(z["center_y"] - expected_title_y))
        candidate_title = min(candidate_zones, key=lambda z: abs(z["center_y"] - expected_title_y))
        def density(path: Path, zone: dict) -> float:
            with Image.open(path) as source:
                ink = 1.0 - np.asarray(source.convert("L"), dtype=np.float64) / 255.0
            crop = ink[zone["top"]:zone["bottom"], zone["left"]:zone["right"]]
            return float(crop.mean()) if crop.size else 0.0
        ref_density, cand_density = density(reference, reference_title), density(candidate, candidate_title)
        reference_body = next((z for z in reference_zones if z["top"] >= reference_title["bottom"]), reference_title)
        candidate_body = next((z for z in candidate_zones if z["top"] >= candidate_title["bottom"]), candidate_title)
        reference_contrast = ref_density / max(0.001, density(reference, reference_body))
        candidate_contrast = cand_density / max(0.001, density(candidate, candidate_body))
        item["title_height_ratio"] = candidate_title["height"] / max(1, reference_title["height"])
        item["title_width_ratio"] = ((candidate_title["right"] - candidate_title["left"]) /
                                     max(1, reference_title["right"] - reference_title["left"]))
        item["title_density_ratio"] = cand_density / max(0.001, ref_density)
        item["reference_title_body_contrast"] = reference_contrast
        item["title_body_contrast"] = candidate_contrast
        item["title_weight_error"] = abs(reference_contrast - candidate_contrast) / max(0.001, reference_contrast)
        observed_weight = "bold" if reference_contrast >= 1.25 else "regular"
        requested_weight = str(parameters["title_weights"].get(str(page), "regular"))
        item["observed_title_weight"] = observed_weight
        item["requested_title_weight"] = requested_weight
        item["title_weight_class_error"] = 0.0 if observed_weight == requested_weight else 1.0
        item["title_typography_error"] = (abs(1 - item["title_height_ratio"]) +
                                           abs(1 - item["title_width_ratio"]) +
                                           abs(1 - item["title_density_ratio"])) / 3
        page_text = next(value["text"] for value in ocr["pages"] if value["page_number"] == page)
        line_report = line_diff_analyzer.analyze(reference, candidate, page_text.splitlines())
        paired_lines = [line for line in line_report["lines"] if line["alignment"] == "paired"]
        item["line_feedback"] = line_report["summary"]
        item["line_reflow_error"] = line_report["summary"]["reflow"] / max(1, len(line_report["lines"]))
        item["line_width_error"] = float(np.median([
            abs(1 - line["width_ratio"]) for line in paired_lines])) if paired_lines else 1.0
        item["line_vertical_error"] = float(np.median([
            abs(line["delta_y_px"]) / raster_height for line in paired_lines])) if paired_lines else 1.0
        metrics.append(item)
    objective = sum(item["horizontal_line_error"] + item["vertical_line_error"]
                    + 2.00 * item["mean_absolute_error"] + 0.08 * item["zone_count_error"]
                    + 0.08 * item["gap_quantile_error"]
                    + 0.15 * item["paragraph_gap_error"]
                    + 0.60 * item["title_typography_error"]
                    + 0.25 * item["title_weight_error"]
                    + 0.05 * item["title_weight_class_error"]
                    + 2.00 * item["body_font_size_error"]
                    + 0.40 * item["line_reflow_error"]
                    + 0.35 * item["line_width_error"]
                    + 0.20 * item["line_vertical_error"]
                    for item in metrics) / len(metrics)
    return {"objective": round(objective, 9), "parameters": copy.deepcopy(parameters),
            "metrics": metrics, "directory": str(trial.resolve()), "pdf": str(pdf.resolve())}


def optimize(args: argparse.Namespace) -> dict:
    root = args.output_dir
    root.mkdir(parents=True, exist_ok=True)
    ocr, profile = json.loads(args.ocr.read_text()), json.loads(args.masters.read_text())
    for page in args.pages:
        command(["pdftoppm", "-f", str(page), "-l", str(page), "-r", str(args.dpi),
                 "-gray", "-png", "-singlefile", str(args.source_pdf),
                 str(root / f"reference-{page:03d}")])
    left = profile["masters"]["left"]
    params = {"size_pt": args.size, "title_size_pt": args.title_size,
              "leading_pt": (args.leading if args.leading is not None else float(left["leading_pt"])),
              "body_width_pt": float(left["body_width_pt"]),
              "top_pt": float(left["top_margin_pt"]), "indent_pt": float(left["first_line_indent_pt"]),
              "paragraph_spacing_pt": args.paragraph_spacing,
              "title_weights": {str(page): args.title_weight for page in args.pages}}
    steps = {"size_pt": 1.0, "title_size_pt": 1.0, "leading_pt": 4.0,
             "body_width_pt": 6.0, "top_pt": 3.0, "indent_pt": 2.0}
    steps["paragraph_spacing_pt"] = max(1.0, args.paragraph_spacing / 2)
    history = []
    best = evaluate(root, args.source_pdf, args.pages, args.dpi, ocr, profile,
                    args.font, args.font_path, params, "cycle-000-base", args.title_font)
    initial_objective = best["objective"]
    history.append({"cycle": 0, **best})
    trial_number = 1
    for cycle in range(1, args.max_cycles + 1):
        cycle_best = best
        for page in args.pages:
            weight_candidate = copy.deepcopy(best["parameters"])
            current_weight = weight_candidate["title_weights"][str(page)]
            weight_candidate["title_weights"][str(page)] = "bold" if current_weight == "regular" else "regular"
            weight_result = evaluate(root, args.source_pdf, args.pages, args.dpi, ocr, profile,
                                     args.font, args.font_path, weight_candidate,
                                     f"trial-{trial_number:03d}-title_weight-{page}", args.title_font)
            trial_number += 1
            if weight_result["objective"] < cycle_best["objective"]:
                cycle_best = weight_result
        for name, step in steps.items():
            for direction in (-1, 1):
                candidate = copy.deepcopy(best["parameters"])
                candidate[name] += direction * step
                if candidate["paragraph_spacing_pt"] < 0:
                    continue
                if min(candidate["size_pt"], candidate["title_size_pt"], candidate["leading_pt"],
                       candidate["body_width_pt"]) <= 0:
                    continue
                result = evaluate(root, args.source_pdf, args.pages, args.dpi, ocr, profile,
                                  args.font, args.font_path, candidate,
                                  f"trial-{trial_number:03d}-{name}", args.title_font)
                trial_number += 1
                if result["objective"] < cycle_best["objective"]:
                    cycle_best = result
        if cycle_best["objective"] >= best["objective"]:
            break
        best = cycle_best
        history.append({"cycle": cycle, **best})
        for name in steps:
            steps[name] *= 0.6
    final_dir = Path(best["directory"])
    shutil.copy2(best["pdf"], root / "reconstruction.pdf")
    shutil.copy2(final_dir / "reconstruction.typ", root / "reconstruction.typ")
    for index in range(1, len(args.pages) + 1):
        shutil.copy2(final_dir / f"candidate-{index}.png", root / f"candidate-{index}.png")
        shutil.copy2(final_dir / f"difference-{index}.png", root / f"difference-{index}.png")
    improved = best["objective"] < initial_objective
    return {"schema_version": 1, "status": "converged" if improved else "not_converged",
            "reason": "objective_reduced" if improved else "no_improvement",
            "initial_objective": initial_objective, "final_objective": best["objective"],
            "improvement": round(initial_objective - best["objective"], 9),
            "cycles_executed": len(history), "evaluations": trial_number,
            "history": history, "selected_parameters": best["parameters"],
            "pdf": str((root / "reconstruction.pdf").resolve())}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ocr", type=Path); parser.add_argument("masters", type=Path)
    parser.add_argument("source_pdf", type=Path); parser.add_argument("--pages", type=int, nargs="+", required=True)
    parser.add_argument("--font", required=True); parser.add_argument("--font-path", type=Path, required=True)
    parser.add_argument("--title-font")
    parser.add_argument("--title-weight", choices=("regular", "bold"), default="regular")
    parser.add_argument("--output-dir", type=Path, required=True); parser.add_argument("--dpi", type=int, default=180)
    parser.add_argument("--size", type=float, default=11.5); parser.add_argument("--title-size", type=float, default=12.8)
    parser.add_argument("--leading", type=float)
    parser.add_argument("--paragraph-spacing", type=float, default=6.0)
    parser.add_argument("--max-cycles", type=int, default=3)
    args = parser.parse_args()
    try:
        result = optimize(args)
        json.dump(result, sys.stdout, ensure_ascii=False, indent=2); sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"book-spread-feedback: {error}", file=sys.stderr); return 2


if __name__ == "__main__":
    raise SystemExit(main())
