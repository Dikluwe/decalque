#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/scan-glyph-segmentation.md
# @updated 2026-09-15
"""Segment a transcribed scan line into labelled glyph samples."""

from __future__ import annotations

import argparse
import json
import sys
import unicodedata
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont
from scipy.ndimage import distance_transform_edt, label as connected_components


def character_weight(character: str) -> float:
    if character.isspace():
        return 0.55
    if character in "i":
        return 0.65
    if character in "lI|!.,:;'`":
        return 0.5
    if character in "rtf":
        return 0.82
    if character in "yo":
        return 1.12
    if character in "ce":
        return 1.05
    if character in "mwMW@%&":
        return 1.45
    if unicodedata.category(character).startswith("P"):
        return 0.65
    if character.isupper():
        return 1.12
    return 1.0


def vertical_seam(ink: np.ndarray, target: float, radius: int) -> tuple[np.ndarray, float]:
    """Find a low-ink top-to-bottom cut near target using dynamic programming."""
    height, width = ink.shape
    low = max(1, int(round(target)) - radius)
    high = min(width - 1, int(round(target)) + radius + 1)
    columns = np.arange(low, high)
    if not len(columns):
        column = min(width - 1, max(1, int(round(target))))
        return np.full(height, column, dtype=np.int32), float(ink[:, column].mean())
    distance = np.abs(columns - target) / max(1, radius)
    pixel_cost = ink[:, columns] * 9.0 + distance[None, :] * 0.32
    costs = pixel_cost[0].copy()
    parents = np.zeros((height, len(columns)), dtype=np.int32)
    for row in range(1, height):
        previous = costs
        costs = np.empty_like(previous)
        for index in range(len(columns)):
            start, stop = max(0, index - 1), min(len(columns), index + 2)
            parent = start + int(np.argmin(previous[start:stop]))
            parents[row, index] = parent
            costs[index] = pixel_cost[row, index] + previous[parent]
    index = int(np.argmin(costs))
    seam = np.empty(height, dtype=np.int32)
    for row in range(height - 1, -1, -1):
        seam[row] = columns[index]
        if row:
            index = parents[row, index]
    active_rows = ink.max(axis=1) > 0.12
    crossing = ink[np.arange(height), seam][active_rows]
    return seam, float((crossing > 0.18).mean()) if crossing.size else 0.0


def isolate_detached_mark(gray: np.ndarray, character: str) -> np.ndarray:
    """Keep the stem and aligned detached mark for i/j, dropping neighbour fragments."""
    if character not in "ij":
        return gray
    ink = gray < 225
    labels, count = connected_components(ink, structure=np.ones((3, 3), dtype=np.uint8))
    components = []
    for identifier in range(1, count + 1):
        rows, columns = np.where(labels == identifier)
        if rows.size:
            components.append({"id": identifier, "area": int(rows.size),
                               "x0": int(columns.min()), "x1": int(columns.max()),
                               "y0": int(rows.min()), "y1": int(rows.max()),
                               "cx": float(columns.mean())})
    if len(components) < 2:
        return gray
    body = max(components, key=lambda item: (item["area"], item["y1"]))
    tolerance = max(2.0, (body["x1"] - body["x0"] + 1) * 0.75)
    candidates = [item for item in components if item is not body and item["y1"] < body["y0"]
                  and body["x0"] - tolerance <= item["cx"] <= body["x1"] + tolerance]
    if not candidates:
        return gray
    mark = min(candidates, key=lambda item: (abs(item["cx"] - body["cx"]), -item["area"]))
    keep = (labels == body["id"]) | (labels == mark["id"])
    isolated = np.full_like(gray, 255); isolated[keep] = gray[keep]
    return isolated


def guide_positions(font: ImageFont.FreeTypeFont, text: str, tracking: float) -> np.ndarray:
    """Return pen positions while preserving the guide font's kerning."""
    positions = [0.0]
    for index in range(1, len(text) + 1):
        positions.append(float(font.getlength(text[:index])) + tracking * index)
    return np.asarray(positions, dtype=np.float64)


def render_guide(text: str, font: ImageFont.FreeTypeFont, positions: np.ndarray,
                 baseline_px: int, shape: tuple[int, int], left: int,
                 scale_x: float) -> np.ndarray:
    height, width = shape
    ascent, _ = font.getmetrics()
    natural_width = max(1, int(np.ceil(positions[-1] + font.size * 0.5)))
    canvas = Image.new("L", (natural_width, height), 0)
    draw = ImageDraw.Draw(canvas)
    for index, character in enumerate(text):
        if not character.isspace():
            draw.text((positions[index], baseline_px - ascent), character, font=font, fill=255)
    scaled_width = max(1, int(round(natural_width * scale_x)))
    canvas = canvas.resize((scaled_width, height), Image.Resampling.BILINEAR)
    result = np.zeros((height, width), dtype=np.uint8)
    stop = min(width, left + scaled_width)
    if stop > left:
        result[:, left:stop] = np.asarray(canvas, dtype=np.uint8)[:, :stop - left]
    return result > 31


def chamfer_error(observed: np.ndarray, candidate: np.ndarray) -> float:
    if not observed.any() or not candidate.any():
        return 1.0
    to_observed = distance_transform_edt(~observed)
    to_candidate = distance_transform_edt(~candidate)
    normalizer = max(1.0, np.sqrt(observed.sum()) * 0.35)
    forward = float(to_observed[candidate].mean()) if candidate.any() else normalizer
    backward = float(to_candidate[observed].mean()) if observed.any() else normalizer
    return min(1.0, (forward + backward) / (2.0 * normalizer))


def fit_guide(font_path: Path, text: str, foreground: np.ndarray, baseline_px: int,
              left_edge: int, right_edge: int) -> dict:
    if not font_path.is_file():
        raise ValueError(f"fonte-guia não encontrada: {font_path}")
    height = foreground.shape[0]
    ink_width = right_edge - left_edge
    best = None
    minimum_size = max(6, int(round(height * 0.45)))
    maximum_size = max(minimum_size, int(round(height * 1.45)))
    for size in range(minimum_size, maximum_size + 1):
        font = ImageFont.truetype(str(font_path), size=size)
        for tracking_ratio in (-0.10, -0.05, 0.0, 0.05, 0.10, 0.15):
            tracking = size * tracking_ratio
            positions = guide_positions(font, text, tracking)
            if positions[-1] <= 0:
                continue
            scale_x = ink_width / positions[-1]
            if not 0.60 <= scale_x <= 1.55:
                continue
            rendered = render_guide(text, font, positions, baseline_px, foreground.shape,
                                    left_edge, scale_x)
            error = chamfer_error(foreground, rendered)
            trial = (error, abs(scale_x - 1.0), abs(tracking_ratio), size,
                     font, positions, scale_x, tracking)
            if best is None or trial[:4] < best[:4]:
                best = trial
    if best is None:
        raise ValueError("não foi possível ajustar a fonte-guia à linha")
    error, _, _, size, font, positions, scale_x, tracking = best
    targets = left_edge + positions[1:-1] * scale_x
    return {"font": font, "positions": positions, "targets": targets,
            "font_size_px": size, "tracking_px": tracking,
            "horizontal_scale": scale_x, "fit_error": error,
            "font_path": str(font_path.resolve())}


def segment(image_path: Path, text: str, baseline_px: int, output_dir: Path,
            uncertain_threshold: float = 0.18, guide_font: Path | None = None) -> dict:
    if not text or not any(not character.isspace() for character in text):
        raise ValueError("text precisa conter ao menos um caractere visível")
    with Image.open(image_path) as source:
        gray = np.asarray(source.convert("L"), dtype=np.uint8)
    if gray.ndim != 2 or min(gray.shape) < 3:
        raise ValueError("imagem de linha inválida")
    height, width = gray.shape
    if not 0 < baseline_px < height:
        raise ValueError("baseline-px deve estar dentro da imagem")
    ink = 1.0 - gray.astype(np.float64) / 255.0
    foreground = ink > 0.12
    if not foreground.any():
        raise ValueError("imagem não contém tinta detectável")
    occupied = np.where(foreground.any(axis=0))[0]
    left_edge, right_edge = max(0, int(occupied[0]) - 1), min(width, int(occupied[-1]) + 2)
    occupied_rows = np.where(foreground.any(axis=1))[0]
    top_edge = max(0, int(occupied_rows[0]) - 1)
    # A line without descenders legitimately ends above its baseline. Keep at least
    # one row below the declared baseline instead of treating that as inconsistent.
    bottom_edge = min(height, max(int(occupied_rows[-1]) + 2, baseline_px + 1))
    local_baseline = baseline_px - top_edge
    if not 0 < local_baseline < bottom_edge - top_edge:
        raise ValueError("baseline-px incompatível com a faixa de tinta")
    guide = None
    if guide_font is not None:
        guide = fit_guide(guide_font, text, foreground, baseline_px, left_edge, right_edge)
        targets = guide["targets"]
    else:
        weights = np.asarray([character_weight(character) for character in text], dtype=np.float64)
        cumulative = np.cumsum(weights)[:-1] / weights.sum()
        targets = left_edge + cumulative * (right_edge - left_edge)
    average = (right_edge - left_edge) / max(1, len(text))
    radius = max(2, int(round(average * 0.48)))
    internal: list[np.ndarray] = []
    crossings: list[float] = []
    for target in targets:
        seam, crossing = vertical_seam(ink, float(target), radius)
        internal.append(seam)
        crossings.append(crossing)
    boundaries = [np.full(height, left_edge, dtype=np.int32), *internal,
                  np.full(height, right_edge, dtype=np.int32)]
    # Preserve ordering even when adjacent search windows touch.
    for row in range(height):
        for index in range(1, len(boundaries)):
            boundaries[index][row] = max(boundaries[index][row], boundaries[index - 1][row] + 1)
        for index in range(len(boundaries) - 2, -1, -1):
            boundaries[index][row] = min(boundaries[index][row], boundaries[index + 1][row] - 1)
    glyph_dir = output_dir / "glyphs"
    glyph_dir.mkdir(parents=True, exist_ok=True)
    samples = []
    occurrences = []
    space_advances = []
    for index, character in enumerate(text):
        left, right = boundaries[index], boundaries[index + 1]
        advance = float(np.median(right - left))
        cut_confidence = 1.0 - max(crossings[index - 1] if index else 0.0,
                                   crossings[index] if index < len(crossings) else 0.0)
        # Above this point the guide is no longer a useful ruler. Real book lines
        # calibrated for this metric remain well below 0.10; 0.18 leaves room for
        # degradation while rejecting geometrically incompatible evidence.
        poor_guide_fit = guide is not None and guide["fit_error"] > 0.18
        status = "uncertain" if (cut_confidence < 1.0 - uncertain_threshold or poor_guide_fit) else "accepted"
        occurrence = {"index": index, "char": character, "advance_px": advance,
                      "left_boundary_px": round(float(np.median(left)), 3),
                      "right_boundary_px": round(float(np.median(right)), 3),
                      "cut_confidence": round(cut_confidence, 6), "status": status}
        if character.isspace():
            space_advances.append(advance)
            occurrence["sample"] = None
            occurrences.append(occurrence)
            continue
        x0, x1 = int(left.min()), int(right.max())
        sample = np.full((bottom_edge - top_edge, max(1, x1 - x0)), 255, dtype=np.uint8)
        for row in range(top_edge, bottom_edge):
            start, stop = int(left[row]), int(right[row])
            sample[row - top_edge, start - x0:stop - x0] = gray[row, start:stop]
        sample = isolate_detached_mark(sample, character)
        filename = f"{index:03d}-U+{ord(character):04X}.png"
        Image.fromarray(sample, mode="L").save(glyph_dir / filename)
        relative = f"glyphs/{filename}"
        occurrence["sample"] = relative
        occurrences.append(occurrence)
        samples.append({"char": character, "path": relative, "baseline_px": local_baseline,
                        "advance_px": advance, "segmentation_status": status,
                        "cut_confidence": round(cut_confidence, 6),
                        "confidence": status,
                        "bounds": {"left_px": x0, "top_px": top_edge,
                                   "right_px": x1, "bottom_px": bottom_edge}})
    visible_advances = [item["advance_px"] for item in samples]
    fallback_space = float(np.median(visible_advances)) * 0.65
    segmentation = {"source": str(image_path.resolve()), "text": text,
                    "method": ("similar-font-guided-variable-seam-v1" if guide is not None
                               else "transcription-guided-variable-seam-v1"),
                    "uncertain_threshold": uncertain_threshold,
                    "occurrences": occurrences,
                    "uncertain_count": sum(item["status"] == "uncertain" for item in occurrences)}
    if guide is not None:
        segmentation["guide"] = {key: round(float(value), 6) if isinstance(value, (float, np.floating)) else value
                                 for key, value in guide.items()
                                 if key in {"font_path", "font_size_px", "tracking_px",
                                            "horizontal_scale", "fit_error"}}
        segmentation["fit_error"] = round(float(guide["fit_error"]), 6)
    manifest = {
        "schema_version": 1,
        "family": "Decalque Scan Derived",
        "style": "Regular",
        "metrics": {"ascender_px": local_baseline,
                    "descender_px": bottom_edge - top_edge - local_baseline,
                    "space_advance_px": float(np.median(space_advances)) if space_advances else fallback_space},
        "samples": samples,
        "segmentation": segmentation,
    }
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest_path = output_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    return {"status": "segmented", "manifest": str(manifest_path.resolve()),
            "sample_count": len(samples), "uncertain_count": manifest["segmentation"]["uncertain_count"]}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("--text", required=True)
    parser.add_argument("--baseline-px", required=True, type=int)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--uncertain-threshold", type=float, default=0.18)
    parser.add_argument("--guide-font", type=Path)
    args = parser.parse_args()
    try:
        result = segment(args.image, args.text, args.baseline_px, args.output_dir,
                         args.uncertain_threshold, args.guide_font)
        json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"scan-glyph-segmenter: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
