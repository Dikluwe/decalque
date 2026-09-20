#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/hybrid-text-reconstruction.md
# @updated 2026-09-16
"""Fit native text to a scan line and preserve only incompatible spans as raster."""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont
from scipy.ndimage import distance_transform_edt, maximum_filter


def load_segmenter():
    path = Path(__file__).with_name("scan_glyph_segmenter.py")
    spec = importlib.util.spec_from_file_location("decalque_scan_segmenter", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


SEGMENTER = load_segmenter()


def mask(gray: np.ndarray) -> np.ndarray:
    return gray < 225


def chamfer(first: np.ndarray, second: np.ndarray) -> float:
    if not first.any() or not second.any():
        return float("inf")
    return float((distance_transform_edt(~first)[second].mean()
                  + distance_transform_edt(~second)[first].mean()) / 2.0)


def diagnostics(observed: np.ndarray, rendered: np.ndarray) -> dict:
    def distribution(values: np.ndarray) -> np.ndarray:
        total = float(values.sum())
        return values.astype(np.float64) / total if total else values.astype(np.float64)
    return {
        "error": chamfer(observed, rendered),
        "horizontal_projection_error": float(np.abs(
            distribution(observed.sum(axis=0)) - distribution(rendered.sum(axis=0))).sum() / 2.0),
        "vertical_projection_error": float(np.abs(
            distribution(observed.sum(axis=1)) - distribution(rendered.sum(axis=1))).sum() / 2.0),
        "unexplained_ink_ratio": 1.0 - float((observed & rendered).sum()) / max(1, int(observed.sum())),
    }


def render_native(text: str, font: ImageFont.FreeTypeFont, positions: np.ndarray,
                  baseline: int, shape: tuple[int, int], left: int, scale_x: float) -> np.ndarray:
    height, width = shape
    ascent, _ = font.getmetrics()
    natural_width = max(1, int(math.ceil(positions[-1] + font.size * 0.5)))
    natural = Image.new("L", (natural_width, height), 255)
    draw = ImageDraw.Draw(natural)
    for index, character in enumerate(text):
        if not character.isspace():
            draw.text((positions[index], baseline - ascent), character, font=font, fill=0)
    scaled = natural.resize((max(1, int(round(natural_width * scale_x))), height),
                            Image.Resampling.LANCZOS)
    result = np.full((height, width), 255, dtype=np.uint8)
    stop = min(width, left + scaled.width)
    if stop > left:
        result[:, left:stop] = np.asarray(scaled, dtype=np.uint8)[:, :stop - left]
    return result


def local_metrics(observed: np.ndarray, rendered: np.ndarray) -> tuple[float, float]:
    if not observed.any():
        return (0.0 if not rendered.any() else float("inf")), 1.0
    error = chamfer(observed, rendered) if rendered.any() else float("inf")
    coverage = float((observed & rendered).sum()) / int(observed.sum())
    return error, coverage


def reconstruct(image_path: Path, text: str, baseline_px: int, font_path: Path,
                output_dir: Path, error_threshold: float = 1.5,
                coverage_threshold: float = 0.42, ink_thinning: float = 0.0) -> dict:
    if not text or not any(not character.isspace() for character in text):
        raise ValueError("texto precisa conter caractere visível")
    if not font_path.is_file():
        raise ValueError(f"fonte não encontrada: {font_path}")
    if not 0.0 <= ink_thinning <= 1.0:
        raise ValueError("ink-thinning deve estar entre 0 e 1")
    with Image.open(image_path) as source:
        gray = np.asarray(source.convert("L"), dtype=np.uint8)
    if gray.ndim != 2 or not 0 < baseline_px < gray.shape[0]:
        raise ValueError("imagem ou baseline inválida")
    observed = mask(gray)
    if not observed.any():
        raise ValueError("linha sem tinta detectável")
    columns = np.where(observed.any(axis=0))[0]
    left, right = int(columns[0]), int(columns[-1]) + 1
    fit = SEGMENTER.fit_guide(font_path, text, observed, baseline_px, left, right)
    positions = fit["positions"]
    pen = left + positions * fit["horizontal_scale"]
    native = render_native(text, fit["font"], positions, baseline_px, gray.shape,
                           left, fit["horizontal_scale"])
    if ink_thinning > 0.0:
        eroded = maximum_filter(native, size=(3, 3), mode="constant", cval=255)
        native = np.clip(np.rint(native.astype(np.float64) * (1.0 - ink_thinning)
                                 + eroded.astype(np.float64) * ink_thinning), 0, 255).astype(np.uint8)
    native_mask = mask(native)
    occurrences = []
    for index, character in enumerate(text):
        x0 = max(0, int(math.floor(pen[index] - 2)))
        x1 = min(gray.shape[1], int(math.ceil(pen[index + 1] + 2)))
        observed_local = observed[:, x0:x1]
        native_local = native_mask[:, x0:x1]
        error, coverage = local_metrics(observed_local, native_local)
        occurrences.append({"index": index, "char": character,
                            "bounds": {"left_px": x0, "top_px": 0,
                                       "right_px": x1, "bottom_px": gray.shape[0]},
                            "local_error": error, "ink_coverage": coverage,
                            "decision": "native"})
    visible = [item for item in occurrences if not item["char"].isspace()
               and math.isfinite(item["local_error"])]
    errors = np.asarray([item["local_error"] for item in visible], dtype=np.float64)
    coverages = np.asarray([item["ink_coverage"] for item in visible], dtype=np.float64)
    median_error = float(np.median(errors)) if errors.size else error_threshold
    median_coverage = float(np.median(coverages)) if coverages.size else coverage_threshold
    error_mad = float(np.median(np.abs(errors - median_error))) if errors.size else 0.0
    coverage_mad = float(np.median(np.abs(coverages - median_coverage))) if coverages.size else 0.0
    adaptive_error = max(error_threshold, median_error + 1.5 * error_mad)
    adaptive_coverage = min(coverage_threshold, median_coverage - 1.5 * coverage_mad)
    for occurrence in occurrences:
        if not occurrence["char"].isspace() and (
                not math.isfinite(occurrence["local_error"])
                or occurrence["local_error"] > adaptive_error
                or occurrence["ink_coverage"] < adaptive_coverage):
            occurrence["decision"] = "raster"
    groups = []
    active = None
    for occurrence in occurrences:
        if occurrence["decision"] == "raster":
            if active is None:
                active = {"start": occurrence["index"], "end": occurrence["index"] + 1}
            else:
                active["end"] = occurrence["index"] + 1
        elif active is not None:
            groups.append(active); active = None
    if active is not None:
        groups.append(active)
    hybrid = native.copy()
    current_error = chamfer(observed, mask(hybrid))
    accepted = []
    raster_dir = output_dir / "raster"
    for group in groups:
        first, last = occurrences[group["start"]], occurrences[group["end"] - 1]
        x0, x1 = first["bounds"]["left_px"], last["bounds"]["right_px"]
        candidate = hybrid.copy()
        candidate[:, x0:x1] = gray[:, x0:x1]
        candidate_error = chamfer(observed, mask(candidate))
        if candidate_error <= current_error + 1e-9:
            raster_dir.mkdir(parents=True, exist_ok=True)
            filename = f"span-{group['start']:03d}-{group['end']:03d}.png"
            Image.fromarray(gray[:, x0:x1], mode="L").save(raster_dir / filename)
            hybrid = candidate
            current_error = candidate_error
            accepted.append({**group, "text": text[group["start"]:group["end"]],
                             "bounds": {"left_px": x0, "top_px": 0,
                                        "right_px": x1, "bottom_px": gray.shape[0]},
                             "path": f"raster/{filename}", "error_after": candidate_error})
        else:
            for index in range(group["start"], group["end"]):
                occurrences[index]["decision"] = "native"
    native_metrics = diagnostics(observed, native_mask)
    hybrid_metrics = diagnostics(observed, mask(hybrid))
    output_dir.mkdir(parents=True, exist_ok=True)
    Image.fromarray(native, mode="L").save(output_dir / "native.png")
    Image.fromarray(hybrid, mode="L").save(output_dir / "hybrid.png")
    residual = np.full(gray.shape + (3,), 255, dtype=np.uint8)
    hybrid_mask = mask(hybrid)
    residual[observed & hybrid_mask] = (0, 0, 0)
    residual[observed & ~hybrid_mask] = (220, 40, 40)
    residual[~observed & hybrid_mask] = (40, 80, 220)
    Image.fromarray(residual, mode="RGB").save(output_dir / "residual.png")
    numeric_fit = {key: round(float(fit[key]), 6)
                   for key in ("font_size_px", "tracking_px", "horizontal_scale", "fit_error")}
    flat_metrics = {"native_error": native_metrics["error"],
                    "hybrid_error": hybrid_metrics["error"],
                    "native_unexplained_ink_ratio": native_metrics["unexplained_ink_ratio"],
                    "hybrid_unexplained_ink_ratio": hybrid_metrics["unexplained_ink_ratio"]}
    plan = {"schema_version": 1, "status": "reconstructed",
            "method": "native-text-with-validated-raster-residuals-v1",
            "source": str(image_path.resolve()), "text": text, "baseline_px": baseline_px,
            "font": str(font_path.resolve()), "fit": numeric_fit,
            "ink_thinning": ink_thinning,
            "thresholds": {"local_error": error_threshold, "ink_coverage": coverage_threshold,
                           "adaptive_local_error": adaptive_error,
                           "adaptive_ink_coverage": adaptive_coverage},
            "occurrences": occurrences, "raster_spans": accepted,
            "raster_intervals": accepted, "metrics": flat_metrics,
            "native_metrics": native_metrics, "hybrid_metrics": hybrid_metrics,
            "native_character_count": sum(item["decision"] == "native" for item in occurrences),
            "raster_character_count": sum(item["decision"] == "raster" for item in occurrences),
            "artifacts": {"native": "native.png", "hybrid": "hybrid.png",
                          "residual": "residual.png"}}
    (output_dir / "plan.json").write_text(json.dumps(plan, ensure_ascii=False, indent=2) + "\n")
    return plan


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("--text", required=True)
    parser.add_argument("--baseline-px", required=True, type=int)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--error-threshold", type=float, default=1.5)
    parser.add_argument("--coverage-threshold", type=float, default=0.42)
    parser.add_argument("--ink-thinning", type=float, default=0.0)
    args = parser.parse_args()
    try:
        plan = reconstruct(args.image, args.text, args.baseline_px, args.font, args.output_dir,
                           args.error_threshold, args.coverage_threshold, args.ink_thinning)
        json.dump({"status": plan["status"], "plan": str((args.output_dir / "plan.json").resolve()),
                   "native_error": plan["native_metrics"]["error"],
                   "hybrid_error": plan["hybrid_metrics"]["error"],
                   "raster_spans": len(plan["raster_spans"])}, sys.stdout, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"hybrid-text-reconstructor: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
