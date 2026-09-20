#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/joint-glyph-reconstruction.md
# @updated 2026-09-16
"""Learn shared raster glyphs by repeatedly reconstructing complete scan lines."""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import sys
from collections import defaultdict
from pathlib import Path

import numpy as np
from PIL import Image
from scipy.ndimage import distance_transform_edt, zoom


def load_segmenter():
    path = Path(__file__).with_name("scan_glyph_segmenter.py")
    spec = importlib.util.spec_from_file_location("decalque_scan_glyph_segmenter", path)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)
    return module


SEGMENTER = load_segmenter()


def ink_mask(gray: np.ndarray) -> np.ndarray:
    return gray < 225


def chamfer(observed: np.ndarray, rendered: np.ndarray) -> float:
    if not observed.any() or not rendered.any():
        return float("inf")
    return float((distance_transform_edt(~observed)[rendered].mean()
                  + distance_transform_edt(~rendered)[observed].mean()) / 2.0)


def projections(observed: np.ndarray, rendered: np.ndarray) -> dict:
    def normalized(values: np.ndarray) -> np.ndarray:
        total = float(values.sum())
        return values.astype(np.float64) / total if total else values.astype(np.float64)
    horizontal = float(np.abs(normalized(observed.sum(axis=0))
                              - normalized(rendered.sum(axis=0))).sum() / 2.0)
    vertical = float(np.abs(normalized(observed.sum(axis=1))
                            - normalized(rendered.sum(axis=1))).sum() / 2.0)
    explained = float((observed & rendered).sum()) / max(1, int(observed.sum()))
    return {"horizontal_projection_error": horizontal,
            "vertical_projection_error": vertical,
            "unexplained_ink_ratio": 1.0 - explained}


def read_corpus(corpus_path: Path, guide_font: Path) -> list[dict]:
    if not guide_font.is_file():
        raise ValueError(f"fonte-guia não encontrada: {guide_font}")
    payload = json.loads(corpus_path.read_text())
    raw_lines = payload.get("lines") if isinstance(payload, dict) else None
    if not isinstance(raw_lines, list) or len(raw_lines) < 2:
        raise ValueError("corpus precisa conter ao menos duas linhas")
    lines = []
    corpus_root = corpus_path.parent
    for index, item in enumerate(raw_lines):
        split = item.get("split")
        if split not in {"train", "validation"}:
            raise ValueError(f"linha {index}: split deve ser train ou validation")
        text = item.get("text", "")
        if not text or not any(not character.isspace() for character in text):
            raise ValueError(f"linha {index}: transcrição visível obrigatória")
        image_path = Path(item.get("image", ""))
        if not image_path.is_absolute():
            image_path = corpus_root / image_path
        with Image.open(image_path) as source:
            gray = np.asarray(source.convert("L"), dtype=np.uint8)
        baseline = int(item.get("baseline_px", 0))
        if gray.ndim != 2 or not 0 < baseline < gray.shape[0] or not ink_mask(gray).any():
            raise ValueError(f"linha {index}: imagem ou baseline inválida")
        foreground = ink_mask(gray)
        columns = np.where(foreground.any(axis=0))[0]
        fit = SEGMENTER.fit_guide(guide_font, text, foreground, baseline,
                                  int(columns[0]), int(columns[-1]) + 1)
        pen = int(columns[0]) + fit["positions"] * fit["horizontal_scale"]
        lines.append({"index": index, "image_path": image_path, "gray": gray,
                      "ink": foreground, "text": text, "baseline": baseline,
                      "split": split, "fit": fit, "pen": pen,
                      "offsets": np.zeros(len(text), dtype=np.float64)})
    if not any(line["split"] == "train" for line in lines):
        raise ValueError("corpus sem linhas de treino")
    if not any(line["split"] == "validation" for line in lines):
        raise ValueError("corpus sem linhas de validação")
    return lines


def occurrence_canvas(line: dict, index: int, reference_size: float,
                      canvas_shape: tuple[int, int], origin_x: int,
                      baseline_y: int) -> tuple[np.ndarray, np.ndarray]:
    """Normalize source ink around one pen origin; return gray and soft responsibility."""
    gray = line["gray"]
    character = line["text"][index]
    size = float(line["fit"]["font_size_px"])
    scale = reference_size / size
    pen = float(line["pen"][index] + line["offsets"][index])
    next_pen = float(line["pen"][index + 1] + line["offsets"][index])
    advance = max(2.0, next_pen - pen)
    pad = max(3.0, size * 0.18)
    x0 = max(0, int(math.floor(pen - pad)))
    x1 = min(gray.shape[1], int(math.ceil(pen + advance + pad)))
    y0 = max(0, int(math.floor(line["baseline"] - size * 1.05)))
    y1 = min(gray.shape[0], int(math.ceil(line["baseline"] + size * 0.35)))
    patch = gray[y0:y1, x0:x1]
    if patch.size == 0:
        return np.full(canvas_shape, 255, dtype=np.uint8), np.zeros(canvas_shape)
    if abs(scale - 1.0) > 0.01:
        patch = zoom(patch, (scale, scale), order=1)
    target = np.full(canvas_shape, 255, dtype=np.uint8)
    weights = np.zeros(canvas_shape, dtype=np.float64)
    tx = int(round(origin_x + (x0 - pen) * scale))
    ty = int(round(baseline_y + (y0 - line["baseline"]) * scale))
    sy0, sx0 = max(0, -ty), max(0, -tx)
    dy0, dx0 = max(0, ty), max(0, tx)
    height = min(patch.shape[0] - sy0, canvas_shape[0] - dy0)
    width = min(patch.shape[1] - sx0, canvas_shape[1] - dx0)
    if height <= 0 or width <= 0:
        return target, weights
    region = patch[sy0:sy0 + height, sx0:sx0 + width]
    target[dy0:dy0 + height, dx0:dx0 + width] = region
    # Pixels near the nominal cell are fully owned; the padded margins taper so
    # neighbour serifs are evidence, but cannot be copied wholesale.
    coordinates = np.arange(x0 + sx0, x0 + sx0 + width, dtype=np.float64)
    distance_outside = np.maximum(pen - coordinates,
                                  coordinates - (pen + advance))
    responsibility = np.where(distance_outside <= 0, 1.0,
                              np.exp(-np.maximum(0.0, distance_outside) / max(1.0, pad * 0.45)))
    if character in "ij":
        responsibility = np.maximum(responsibility, 0.75)
    weights[dy0:dy0 + height, dx0:dx0 + width] = responsibility[None, :]
    return target, weights


def build_atlas(lines: list[dict], reference_size: float, canvas_shape: tuple[int, int],
                origin_x: int, baseline_y: int) -> tuple[dict[str, np.ndarray], dict[str, int]]:
    evidence: dict[str, list[tuple[np.ndarray, np.ndarray]]] = defaultdict(list)
    for line in lines:
        if line["split"] != "train":
            continue
        for index, character in enumerate(line["text"]):
            if character.isspace():
                continue
            evidence[character].append(occurrence_canvas(
                line, index, reference_size, canvas_shape, origin_x, baseline_y))
    atlas = {}
    counts = {}
    for character, occurrences in evidence.items():
        darkness = np.stack([255.0 - gray.astype(np.float64) for gray, _ in occurrences])
        weights = np.stack([weight for _, weight in occurrences])
        weighted = darkness * weights
        denominator = weights.sum(axis=0)
        mean_darkness = np.divide(weighted.sum(axis=0), denominator,
                                  out=np.zeros(canvas_shape), where=denominator > 1e-8)
        support = (weighted > 30.0).sum(axis=0)
        required = max(1, int(math.ceil(len(occurrences) * 0.35)))
        mean_darkness[support < required] = 0.0
        # Sparse punctuation can be attenuated by probabilistic ownership until
        # no SDF interior remains. Restore only such faint atlases; ordinary
        # glyphs retain the measured source intensity unchanged.
        peak = float(mean_darkness.max())
        if 0.0 < peak < 160.0:
            mean_darkness *= 160.0 / peak
        atlas[character] = np.clip(np.rint(255.0 - mean_darkness), 0, 255).astype(np.uint8)
        counts[character] = len(occurrences)
    return atlas, counts


def stamp(canvas: np.ndarray, glyph: np.ndarray, pen_x: float, baseline: int,
          reference_size: float, line_size: float, origin_x: int, baseline_y: int) -> None:
    scale = line_size / reference_size
    rendered = glyph
    if abs(scale - 1.0) > 0.01:
        rendered = zoom(glyph, (scale, scale), order=1)
    x0 = int(round(pen_x - origin_x * scale))
    y0 = int(round(baseline - baseline_y * scale))
    sy0, sx0 = max(0, -y0), max(0, -x0)
    dy0, dx0 = max(0, y0), max(0, x0)
    height = min(rendered.shape[0] - sy0, canvas.shape[0] - dy0)
    width = min(rendered.shape[1] - sx0, canvas.shape[1] - dx0)
    if height > 0 and width > 0:
        region = rendered[sy0:sy0 + height, sx0:sx0 + width]
        canvas[dy0:dy0 + height, dx0:dx0 + width] = np.minimum(
            canvas[dy0:dy0 + height, dx0:dx0 + width], region)


def render_line(line: dict, atlas: dict[str, np.ndarray], reference_size: float,
                origin_x: int, baseline_y: int) -> np.ndarray:
    canvas = np.full(line["gray"].shape, 255, dtype=np.uint8)
    size = float(line["fit"]["font_size_px"])
    for index, character in enumerate(line["text"]):
        if character in atlas and not character.isspace():
            stamp(canvas, atlas[character], line["pen"][index] + line["offsets"][index],
                  line["baseline"], reference_size, size, origin_x, baseline_y)
    return canvas


def evaluate(lines: list[dict], atlas: dict[str, np.ndarray], reference_size: float,
             origin_x: int, baseline_y: int) -> tuple[dict, dict[int, np.ndarray]]:
    by_split = defaultdict(list)
    diagnostics = defaultdict(list)
    renders = {}
    for line in lines:
        rendered = render_line(line, atlas, reference_size, origin_x, baseline_y)
        renders[line["index"]] = rendered
        observed_mask, rendered_mask = line["ink"], ink_mask(rendered)
        error = chamfer(observed_mask, rendered_mask)
        by_split[line["split"]].append(error)
        diagnostics[line["split"]].append(projections(observed_mask, rendered_mask))
    result = {}
    for split in ("train", "validation"):
        result[f"{split}_error"] = float(np.mean(by_split[split]))
        for key in diagnostics[split][0]:
            result[f"{split}_{key}"] = float(np.mean([item[key] for item in diagnostics[split]]))
    return result, renders


def refine_offsets(lines: list[dict], atlas: dict[str, np.ndarray], reference_size: float,
                   origin_x: int, baseline_y: int) -> None:
    """Coordinate descent over occurrence positions using local ink distance."""
    for line in lines:
        if line["split"] != "train":
            continue
        size = float(line["fit"]["font_size_px"])
        for index, character in enumerate(line["text"]):
            if character.isspace() or character not in atlas:
                continue
            pen = float(line["pen"][index])
            advance = max(3, int(round(line["pen"][index + 1] - pen)))
            left = max(0, int(pen - 5)); right = min(line["gray"].shape[1], int(pen + advance + 5))
            observed = line["ink"][:, left:right]
            best = None
            for delta in (-2.0, -1.0, 0.0, 1.0, 2.0):
                candidate = np.full(line["gray"].shape, 255, dtype=np.uint8)
                stamp(candidate, atlas[character], pen + delta, line["baseline"],
                      reference_size, size, origin_x, baseline_y)
                rendered = ink_mask(candidate)[:, left:right]
                score = chamfer(observed, rendered)
                trial = (score, abs(delta), delta)
                if best is None or trial < best:
                    best = trial
            line["offsets"][index] = best[2]


def reconstruct(corpus_path: Path, guide_font: Path, output_dir: Path,
                max_iterations: int = 4) -> dict:
    if max_iterations < 1:
        raise ValueError("iterations deve ser positivo")
    lines = read_corpus(corpus_path, guide_font)
    reference_size = float(np.median([line["fit"]["font_size_px"] for line in lines
                                      if line["split"] == "train"]))
    canvas_shape = (max(40, int(round(reference_size * 1.55))),
                    max(44, int(round(reference_size * 1.8))))
    origin_x = max(7, int(round(reference_size * 0.28)))
    baseline_y = max(25, int(round(reference_size * 1.12)))
    history = []
    best = None
    best_atlas = None
    best_renders = None
    counts = {}
    for iteration in range(max_iterations):
        atlas, counts = build_atlas(lines, reference_size, canvas_shape, origin_x, baseline_y)
        if not atlas:
            raise ValueError("nenhum glifo de treino pôde ser reconstruído")
        metrics, renders = evaluate(lines, atlas, reference_size, origin_x, baseline_y)
        record = {"iteration": iteration, **metrics}
        history.append(record)
        if best is None or metrics["validation_error"] < best["validation_error"] - 1e-9:
            best = record
            best_atlas = {character: glyph.copy() for character, glyph in atlas.items()}
            best_renders = {index: image.copy() for index, image in renders.items()}
        else:
            break
        refine_offsets(lines, atlas, reference_size, origin_x, baseline_y)
    assert best is not None and best_atlas is not None and best_renders is not None
    output_dir.mkdir(parents=True, exist_ok=True)
    glyph_dir = output_dir / "glyphs"; glyph_dir.mkdir(exist_ok=True)
    reconstruction_dir = output_dir / "reconstructions"; reconstruction_dir.mkdir(exist_ok=True)
    samples = []
    advances = defaultdict(list)
    for line in lines:
        scale = reference_size / float(line["fit"]["font_size_px"])
        for index, character in enumerate(line["text"]):
            if not character.isspace():
                advances[character].append(float(line["pen"][index + 1] - line["pen"][index]) * scale)
    for character in sorted(best_atlas, key=ord):
        filename = f"U+{ord(character):04X}.png"
        advance = float(np.median(advances[character]))
        export_width = max(2, int(math.ceil(advance)))
        source_glyph = best_atlas[character]
        ink_columns = np.where((source_glyph < 250).any(axis=0))[0]
        export_left = int(ink_columns[0]) if ink_columns.size else origin_x
        glyph = source_glyph[:, export_left:export_left + export_width]
        if glyph.shape[1] < export_width:
            glyph = np.pad(glyph, ((0, 0), (0, export_width - glyph.shape[1])),
                           constant_values=255)
        Image.fromarray(glyph, mode="L").save(glyph_dir / filename)
        samples.append({"char": character, "path": f"glyphs/{filename}",
                        "baseline_px": baseline_y,
                        "advance_px": advance,
                        "occurrence_count": counts[character], "confidence": "accepted"})
    for line in lines:
        Image.fromarray(best_renders[line["index"]], mode="L").save(
            reconstruction_dir / f"line-{line['index']:03d}-{line['split']}.png")
    space_advances = []
    for line in lines:
        scale = reference_size / float(line["fit"]["font_size_px"])
        for index, character in enumerate(line["text"]):
            if character.isspace():
                space_advances.append(float(line["pen"][index + 1] - line["pen"][index]) * scale)
    fallback_space = reference_size * 0.3
    manifest = {"schema_version": 1, "family": "Decalque Joint Scan Derived",
                "style": "Regular", "metrics": {"ascender_px": baseline_y,
                "descender_px": canvas_shape[0] - baseline_y,
                "space_advance_px": float(np.median(space_advances)) if space_advances else fallback_space},
                "samples": samples}
    (output_dir / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    report = {"schema_version": 1, "status": "reconstructed",
              "method": "joint-line-feedback-raster-atlas-v1",
              "corpus": str(corpus_path.resolve()), "guide_font": str(guide_font.resolve()),
              "line_count": len(lines), "train_line_count": sum(line["split"] == "train" for line in lines),
              "validation_line_count": sum(line["split"] == "validation" for line in lines),
              "reference_size_px": reference_size, "canvas": {"width": canvas_shape[1],
              "height": canvas_shape[0], "origin_x": origin_x, "baseline_y": baseline_y},
              "coverage": "".join(sorted(best_atlas)), "occurrences": counts,
              "history": history, "best_iteration": best["iteration"], "best": best,
              "limitations": ["single-horizontal-style", "approximately-uniform-size",
                              "raster-atlas-before-vectorization", "guide-used-only-for-geometry"]}
    (output_dir / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("corpus", type=Path)
    parser.add_argument("--guide-font", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--iterations", type=int, default=4)
    args = parser.parse_args()
    try:
        report = reconstruct(args.corpus, args.guide_font, args.output_dir, args.iterations)
        json.dump({"status": report["status"], "report": str((args.output_dir / "report.json").resolve()),
                   "best_iteration": report["best_iteration"], "validation_error": report["best"]["validation_error"]},
                  sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"joint-glyph-reconstructor: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
