#!/usr/bin/env python3
# Crystalline Lineage
# @prompt 00_nucleo/prompts/scan-derived-font.md
# @layer L5
# @updated 2026-09-15
"""Build an experimental TrueType font from explicitly labelled scan glyph samples."""

from __future__ import annotations

import argparse
import json
import statistics
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from scipy.ndimage import distance_transform_edt


def glyph_name(character: str) -> str:
    codepoints = "_".join(f"{ord(value):04X}" for value in character)
    return f"uni{codepoints}"


def load_manifest(path: Path) -> dict[str, Any]:
    source = json.loads(path.read_text())
    if source.get("schema_version") != 1:
        raise ValueError("schema_version não suportada")
    if not source.get("family") or not isinstance(source.get("samples"), list) or not source["samples"]:
        raise ValueError("family e samples não vazios são obrigatórios")
    metrics = source.get("metrics", {})
    if min(float(metrics.get("ascender_px", 0)), float(metrics.get("descender_px", 0))) <= 0:
        raise ValueError("ascender_px e descender_px devem ser positivos")
    for sample in source["samples"]:
        if not isinstance(sample.get("char"), str) or len(sample["char"]) != 1:
            raise ValueError("cada amostra exige exatamente um caractere Unicode")
        if float(sample.get("baseline_px", -1)) < 0 or float(sample.get("advance_px", 0)) <= 0:
            raise ValueError("baseline_px e advance_px inválidos")
    return source


def sample_ink(manifest_path: Path, sample: dict[str, Any]) -> np.ndarray:
    path = (manifest_path.parent / sample["path"]).resolve()
    with Image.open(path) as source:
        gray = np.asarray(source.convert("L"), dtype=np.float64)
    return 1.0 - gray / 255.0


def aligned_layers(samples: list[tuple[np.ndarray, dict[str, Any]]], ascender_px: int,
                   descender_px: int) -> tuple[list[np.ndarray], float]:
    baseline = ascender_px
    width = max(int(round(meta["advance_px"])) for _, meta in samples)
    height = ascender_px + descender_px
    layers = []
    for ink, meta in samples:
        canvas = np.zeros((height, width), dtype=np.float64)
        top = baseline - int(round(meta["baseline_px"]))
        if "left_bearing_px" in meta:
            # Samples may retain page-space padding. Move their observed bearing
            # back to the common glyph origin before combining occurrences.
            left = -int(round(meta["left_bearing_px"]))
        elif len(samples) > 1 and np.any(ink >= 0.12):
            columns = np.where(ink >= 0.12)[1]
            ink_center = (float(columns.min()) + float(columns.max()) + 1.0) / 2.0
            left = int(round(width / 2.0 - ink_center))
        else:
            left = 0
        y0, x0 = max(0, top), max(0, left)
        sy, sx = max(0, -top), max(0, -left)
        copy_height = min(ink.shape[0] - sy, height - y0)
        copy_width = min(ink.shape[1] - sx, width - x0)
        if copy_height > 0 and copy_width > 0:
            canvas[y0:y0 + copy_height, x0:x0 + copy_width] = ink[sy:sy + copy_height, sx:sx + copy_width]
        layers.append(canvas)
    return layers, statistics.median(float(meta["advance_px"]) for _, meta in samples)


def aggregate(samples: list[tuple[np.ndarray, dict[str, Any]]], ascender_px: int,
              descender_px: int) -> tuple[np.ndarray, float]:
    layers, advance = aligned_layers(samples, ascender_px, descender_px)
    return np.median(np.stack(layers), axis=0) >= 0.35, advance


def aggregate_sdf(samples: list[tuple[np.ndarray, dict[str, Any]]], ascender_px: int,
                  descender_px: int) -> tuple[np.ndarray, float, int]:
    if all(np.all(ink >= 0.5) for ink, _ in samples):
        raise ValueError("isoline SDF ausente: amostra visível não contém exterior")
    layers, advance = aligned_layers(samples, ascender_px, descender_px)
    fields = []
    for ink in layers:
        inside = ink >= 0.5
        # Negative values are inside the observed ink. Keep the distance geometric:
        # adding grayscale intensity here shifts the zero isoline and thickens strokes.
        field = distance_transform_edt(~inside) - distance_transform_edt(inside)
        fields.append(field)
    median = np.median(np.stack(fields), axis=0)
    if not np.any(median <= 0) or not np.any(median > 0):
        raise ValueError("isoline SDF ausente: a amostra não contém interior e exterior")
    return median, advance, int((median <= 0).sum())


def raster_glyph(binary: np.ndarray, scale: float, baseline_row: int):
    pen = TTGlyphPen(None)
    for row, pixels in enumerate(binary):
        start = None
        for column, active in enumerate(np.append(pixels, False)):
            if active and start is None:
                start = column
            elif start is not None and not active:
                x0, x1 = round(start * scale), round(column * scale)
                y1, y0 = round((baseline_row - row) * scale), round((baseline_row - row - 1) * scale)
                if x1 > x0 and y1 > y0:
                    pen.moveTo((x0, y0)); pen.lineTo((x1, y0)); pen.lineTo((x1, y1)); pen.lineTo((x0, y1)); pen.closePath()
                start = None
    return pen.glyph()


def _edge_point(row: int, column: int, edge: int, values: tuple[float, float, float, float]):
    top_left, top_right, bottom_right, bottom_left = values
    endpoints = ((top_left, top_right, (column, row), (column + 1, row)),
                 (top_right, bottom_right, (column + 1, row), (column + 1, row + 1)),
                 (bottom_left, bottom_right, (column, row + 1), (column + 1, row + 1)),
                 (top_left, bottom_left, (column, row), (column, row + 1)))
    first, second, start, stop = endpoints[edge]
    denominator = first - second
    fraction = 0.5 if abs(denominator) < 1e-12 else first / denominator
    fraction = min(1.0, max(0.0, fraction))
    return (start[0] + (stop[0] - start[0]) * fraction,
            start[1] + (stop[1] - start[1]) * fraction)


def sdf_segments(field: np.ndarray):
    base_cases = {1: [(3, 0)], 2: [(0, 1)], 3: [(3, 1)], 4: [(1, 2)],
                  6: [(0, 2)], 7: [(3, 2)], 8: [(2, 3)], 9: [(0, 2)],
                  11: [(1, 2)], 12: [(1, 3)], 13: [(0, 1)], 14: [(3, 0)]}
    segments = []
    for row in range(field.shape[0] - 1):
        for column in range(field.shape[1] - 1):
            values = (float(field[row, column]), float(field[row, column + 1]),
                      float(field[row + 1, column + 1]), float(field[row + 1, column]))
            case = sum(bit for bit, value in zip((1, 2, 4, 8), values) if value <= 0)
            pairs = base_cases.get(case, [])
            if case in (5, 10):
                center_inside = sum(values) / 4 <= 0
                if case == 5:
                    pairs = [(0, 1), (2, 3)] if center_inside else [(3, 0), (1, 2)]
                else:
                    pairs = [(3, 0), (1, 2)] if center_inside else [(0, 1), (2, 3)]
            for first, second in pairs:
                segments.append((_edge_point(row, column, first, values),
                                 _edge_point(row, column, second, values)))
    return segments


def stitch_contours(segments: list[tuple[tuple[float, float], tuple[float, float]]]):
    def key(point): return round(point[0], 6), round(point[1], 6)
    adjacency: dict[tuple[float, float], list[tuple[float, float]]] = defaultdict(list)
    points = {}; unused = set()
    for first, second in segments:
        a, b = key(first), key(second)
        if a == b:
            continue
        adjacency[a].append(b); adjacency[b].append(a); points[a] = first; points[b] = second
        unused.add(frozenset((a, b)))
    contours = []
    while unused:
        edge = next(iter(unused)); start, current = tuple(edge); previous = start
        contour = [points[start]]; unused.remove(edge)
        while current != start:
            contour.append(points[current])
            candidates = [candidate for candidate in adjacency[current]
                          if frozenset((current, candidate)) in unused]
            if not candidates: break
            following = candidates[0] if len(candidates) == 1 else next(
                (candidate for candidate in candidates if candidate != previous), candidates[0])
            unused.remove(frozenset((current, following)))
            previous, current = current, following
        if current == start and len(contour) >= 4:
            contours.append(contour)
    return contours


def _point_line_distance(point, start, stop):
    vector = np.asarray(stop) - np.asarray(start)
    if np.allclose(vector, 0): return float(np.linalg.norm(np.asarray(point) - start))
    return float(abs(np.cross(vector, np.asarray(start) - point)) / np.linalg.norm(vector))


def simplify_open(points, tolerance: float):
    if len(points) <= 2: return points
    distances = [_point_line_distance(point, points[0], points[-1]) for point in points[1:-1]]
    if not distances or max(distances) <= tolerance: return [points[0], points[-1]]
    index = 1 + int(np.argmax(distances))
    return simplify_open(points[:index + 1], tolerance)[:-1] + simplify_open(points[index:], tolerance)


def simplify_closed(points, tolerance: float):
    if len(points) < 6: return points
    anchor = max(range(1, len(points)), key=lambda index: np.linalg.norm(
        np.asarray(points[index]) - np.asarray(points[0])))
    first = simplify_open(points[:anchor + 1], tolerance)
    second = simplify_open(points[anchor:] + [points[0]], tolerance)
    return (first[:-1] + second[:-1]) if len(first) + len(second) > 4 else points


def sdf_glyph(field: np.ndarray, scale: float, baseline_row: int):
    outside = max(1.0, float(np.max(field)) + 1.0)
    padded = np.pad(field, 1, mode="constant", constant_values=outside)
    contours = stitch_contours(sdf_segments(padded))
    pen = TTGlyphPen(None); point_count = 0
    for raw in contours:
        contour = simplify_closed(raw, 0.35)
        if len(contour) < 3: continue
        converted = [(round((x - 1) * scale), round((baseline_row + 1 - y) * scale))
                     for x, y in contour]
        # The SDF already supplies subpixel points. Preserve them as on-curve vertices;
        # unconditional quadratic smoothing erases real serif corners in sparse evidence.
        pen.moveTo(converted[0])
        for point in converted[1:]:
            pen.lineTo(point)
        pen.closePath(); point_count += len(converted)
    if not contours or point_count == 0:
        raise ValueError("isolinha SDF não produziu contorno fechado")
    return pen.glyph(), len(contours), point_count


def notdef_glyph(units_per_em: int):
    pen = TTGlyphPen(None); inset = units_per_em // 10
    pen.moveTo((inset, 0)); pen.lineTo((units_per_em - inset, 0)); pen.lineTo((units_per_em - inset, units_per_em))
    pen.lineTo((inset, units_per_em)); pen.closePath()
    pen.moveTo((inset * 2, inset)); pen.lineTo((inset * 2, units_per_em - inset))
    pen.lineTo((units_per_em - inset * 2, units_per_em - inset)); pen.lineTo((units_per_em - inset * 2, inset)); pen.closePath()
    return pen.glyph()


def build(manifest_path: Path, output_path: Path, vectorization: str = "scanline") -> dict[str, Any]:
    source = load_manifest(manifest_path); metrics = source["metrics"]
    units = int(source.get("units_per_em", 1000)); asc_px = int(round(metrics["ascender_px"])); desc_px = int(round(metrics["descender_px"]))
    if not 256 <= units <= 16384:
        raise ValueError("units_per_em fora do intervalo TrueType")
    scale = units / (asc_px + desc_px)
    grouped: dict[str, list[tuple[np.ndarray, dict[str, Any]]]] = defaultdict(list)
    for sample in source["samples"]:
        grouped[sample["char"]].append((sample_ink(manifest_path, sample), sample))
    characters = sorted(grouped, key=ord); names = {char: glyph_name(char) for char in characters}
    order = [".notdef", "space"] + [names[char] for char in characters if char != " "]
    glyphs = {".notdef": notdef_glyph(units), "space": TTGlyphPen(None).glyph()}
    horizontal = {".notdef": (units, 0), "space": (round(float(metrics.get("space_advance_px", asc_px / 2)) * scale), 0)}
    evidence = []
    for char in characters:
        if char == " ": continue
        contour_count = point_count = 0
        if vectorization == "sdf-contour":
            try:
                field, advance, ink_pixels = aggregate_sdf(grouped[char], asc_px, desc_px)
                glyph, contour_count, point_count = sdf_glyph(field, scale, asc_px)
            except ValueError as error:
                raise ValueError(f"glifo {char!r}: {error}") from error
        else:
            binary, advance = aggregate(grouped[char], asc_px, desc_px)
            glyph, ink_pixels = raster_glyph(binary, scale, asc_px), int(binary.sum())
        name = names[char]; glyphs[name] = glyph
        horizontal[name] = (max(1, round(advance * scale)), 0)
        evidence.append({"char": char, "glyph": name, "sample_count": len(grouped[char]),
                         "advance_px": advance, "advance_units": horizontal[name][0],
                         "ink_pixels": ink_pixels, "aggregated_ink_pixels": ink_pixels,
                         "contour_count": contour_count,
                         "point_count": point_count})
    cmap = {32: "space", **{ord(char): names[char] for char in characters if char != " "}}
    builder = FontBuilder(units, isTTF=True); builder.setupGlyphOrder(order); builder.setupCharacterMap(cmap)
    builder.setupGlyf(glyphs); builder.setupHorizontalMetrics(horizontal)
    asc_units, desc_units = round(asc_px * scale), -round(desc_px * scale)
    builder.setupHorizontalHeader(ascent=asc_units, descent=desc_units)
    builder.setupNameTable({"familyName": source["family"], "styleName": source.get("style", "Regular"),
                            "uniqueFontIdentifier": f'{source["family"]}-{source.get("style", "Regular")}-scan-derived',
                            "fullName": f'{source["family"]} {source.get("style", "Regular")}',
                            "psName": f'{source["family"].replace(" ", "")}-{source.get("style", "Regular")}'} )
    builder.setupOS2(sTypoAscender=asc_units, sTypoDescender=desc_units,
                     usWinAscent=max(0, asc_units), usWinDescent=max(0, -desc_units))
    builder.setupPost(); builder.setupMaxp(); output_path.parent.mkdir(parents=True, exist_ok=True); builder.save(output_path)
    return {"schema_version": 1, "status": "built", "font": str(output_path.resolve()),
            "family": source["family"], "style": source.get("style", "Regular"),
            "units_per_em": units, "pixel_to_unit_scale": round(scale, 6),
            "vectorization": vectorization,
            "coverage": characters, "glyph_count": len(order), "evidence": evidence,
            "limitations": ["scanline-vectorization", "labelled-samples-required", "kerning-not-yet-derived"]}


def main() -> int:
    parser = argparse.ArgumentParser(); parser.add_argument("manifest", type=Path); parser.add_argument("output", type=Path)
    parser.add_argument("--vectorization", choices=("scanline", "sdf-contour"), default="scanline")
    args = parser.parse_args()
    try:
        result = build(args.manifest, args.output, args.vectorization); json.dump(result, sys.stdout, ensure_ascii=False, indent=2); sys.stdout.write("\n"); return 0
    except Exception as error:
        print(f"scan-font-builder: {error}", file=sys.stderr); return 2


if __name__ == "__main__": raise SystemExit(main())
