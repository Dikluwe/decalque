#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/local-font-discovery.md
# @updated 2026-09-15
"""Rank local font families against raster evidence without network access."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable

from fontTools.ttLib import TTCollection, TTFont
import numpy as np
from PIL import Image, ImageDraw, ImageFont

from google_fonts_discovery import load_evidence, normalized_ink, similarity, weighted_score


FONT_SUFFIXES = {".ttf", ".otf", ".ttc", ".otc"}
CANONICAL_SIZE_PX = 180


def fontconfig_paths() -> list[Path]:
    process = subprocess.run(["fc-list", "--format", "%{file}\n"], capture_output=True,
                             text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or "fc-list falhou")
    return sorted({Path(line).resolve() for line in process.stdout.splitlines() if line.strip()},
                  key=lambda path: str(path))


def directory_paths(directories: Iterable[Path]) -> dict[Path, str]:
    paths = {}
    for directory in directories:
        if not directory.is_dir():
            raise ValueError(f"diretório de fontes inexistente: {directory}")
        for path in directory.rglob("*"):
            if path.is_file() and path.suffix.lower() in FONT_SUFFIXES:
                paths.setdefault(path.resolve(), f"explicit:{directory.resolve()}")
    return dict(sorted(paths.items(), key=lambda item: str(item[0])))


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def name_value(font: TTFont, identifier: int) -> str | None:
    values = []
    for record in font["name"].names:
        if record.nameID == identifier:
            try:
                value = record.toUnicode().strip()
            except Exception:
                continue
            if value:
                values.append(value)
    if not values:
        return None
    return sorted(set(values), key=lambda value: (0 if value.isascii() else 1, len(value), value))[0]


def weight_number(font: TTFont, style: str) -> int:
    if "OS/2" in font:
        value = int(font["OS/2"].usWeightClass)
        if 1 <= value <= 1000:
            return value
    lowered = style.lower().replace(" ", "")
    for token, value in (("thin", 100), ("extralight", 200), ("ultralight", 200),
                         ("light", 300), ("medium", 500), ("semibold", 600),
                         ("demibold", 600), ("extrabold", 800), ("ultrabold", 800),
                         ("black", 900), ("heavy", 900), ("bold", 700)):
        if token in lowered:
            return value
    return 400


def variant_tags(weight: int, italic: bool) -> set[str]:
    numeric = str(weight)
    tags = {numeric + ("italic" if italic else "")}
    if italic:
        tags.add("italic")
    elif weight == 400:
        tags.add("regular")
    return tags


def face_metadata(path: Path, index: int, font: TTFont, sha256: str, origin: str) -> dict[str, Any]:
    family = name_value(font, 1) or name_value(font, 16) or path.stem
    style = name_value(font, 2) or name_value(font, 17) or "Regular"
    italic = "italic" in style.lower() or "oblique" in style.lower()
    weight = weight_number(font, style)
    units_per_em = int(font["head"].unitsPerEm) if "head" in font else None
    m_advance_em = None
    if units_per_em and "hmtx" in font:
        glyph = (font.getBestCmap() or {}).get(ord("M"))
        if glyph in font["hmtx"].metrics:
            m_advance_em = round(font["hmtx"].metrics[glyph][0] / units_per_em, 6)
    return {"family": family, "style": style, "weight": weight, "italic": italic,
            "variants": sorted(variant_tags(weight, italic)), "path": str(path), "face_index": index,
            "sha256": sha256, "origin": origin, "license": name_value(font, 13),
            "license_url": name_value(font, 14), "copyright": name_value(font, 0),
            "units_per_em": units_per_em, "m_advance_em": m_advance_em}


def inspect_font(path: Path, origin: str) -> list[dict[str, Any]]:
    sha256 = file_sha256(path)
    fonts: list[TTFont] = []
    collection = None
    try:
        if path.suffix.lower() in {".ttc", ".otc"}:
            collection = TTCollection(path, lazy=True)
            fonts = list(collection.fonts)
        else:
            fonts = [TTFont(path, lazy=True)]
        return [face_metadata(path, index, font, sha256, origin)
                for index, font in enumerate(fonts)]
    finally:
        for font in fonts:
            font.close()
        if collection is not None:
            collection.close()


def render_text(text: str, face: dict[str, Any], size_px: int = CANONICAL_SIZE_PX) -> Image.Image:
    font = ImageFont.truetype(face["path"], size_px, index=face["face_index"])
    probe = Image.new("L", (1, 1), 255)
    bounds = ImageDraw.Draw(probe).textbbox((0, 0), text, font=font)
    image = Image.new("L", (bounds[2] - bounds[0] + 40, bounds[3] - bounds[1] + 40), 255)
    ImageDraw.Draw(image).text((20 - bounds[0], 20 - bounds[1]), text, font=font, fill=0)
    return image


def ink_metrics(image: Image.Image) -> dict[str, Any] | None:
    array = np.asarray(image.convert("L"), dtype=np.float64)
    ink = 1.0 - array / 255.0
    active = np.argwhere(ink > 0.08)
    if not len(active):
        return None
    y0, x0 = active.min(axis=0)
    y1, x1 = active.max(axis=0) + 1
    crop = ink[int(y0):int(y1), int(x0):int(x1)]
    return {"bbox_px": [int(x0), int(y0), int(x1), int(y1)],
            "ink_width_px": int(x1 - x0), "ink_height_px": int(y1 - y0),
            "ink_density": round(float(crop.mean()), 6)}


def estimate_geometry(observed: dict[str, Any], rendered: dict[str, Any], text: str,
                      pt_per_px: float | None) -> dict[str, Any]:
    height_scale = observed["ink_height_px"] / rendered["ink_height_px"]
    estimated_size_px = CANONICAL_SIZE_PX * height_scale
    predicted_width = rendered["ink_width_px"] * height_scale
    width_scale = observed["ink_width_px"] / predicted_width if predicted_width else None
    gaps = len(text) - 1
    tracking = None
    if gaps > 0 and estimated_size_px > 0:
        tracking = (observed["ink_width_px"] - predicted_width) / (gaps * estimated_size_px)
    return {"observed_ink": observed, "canonical_render_ink": rendered,
            "canonical_size_px": CANONICAL_SIZE_PX,
            "estimated_size_px": round(estimated_size_px, 6),
            "estimated_size_pt": (round(estimated_size_px * pt_per_px, 6)
                                  if pt_per_px is not None else None),
            "estimated_tracking_em": round(tracking, 6) if tracking is not None else None,
            "width_scale_without_tracking": round(width_scale, 6) if width_scale is not None else None}


def matches_variant(face: dict[str, Any], allowed: list[str]) -> bool:
    return bool(set(allowed) & set(face["variants"]))


def discover(evidence_path: Path, directories: list[Path], threshold: float,
             limit: int, use_fontconfig: bool, pt_per_px: float | None = None) -> dict[str, Any]:
    if not 0 <= threshold <= 1:
        raise ValueError("threshold deve estar entre 0 e 1")
    if limit < 1:
        raise ValueError("limit deve ser positivo")
    if pt_per_px is not None and pt_per_px <= 0:
        raise ValueError("pt-per-px deve ser positivo")
    samples = load_evidence(evidence_path)
    target_inks, observed_geometry = {}, {}
    for sample in samples:
        with Image.open(sample["path"]) as image:
            target_inks[sample["id"]] = normalized_ink(image)
            observed_geometry[sample["id"]] = ink_metrics(image)

    if directories:
        origins = directory_paths(directories)
        paths = list(origins)
        effective_dirs = [str(path.resolve()) for path in directories]
    elif use_fontconfig:
        paths = fontconfig_paths()
        origins = {path: "fontconfig" for path in paths}
        effective_dirs = ["fontconfig"]
    else:
        paths, origins, effective_dirs = [], {}, []

    faces, rejected, seen_hashes = [], [], set()
    for path in paths:
        try:
            sha256 = file_sha256(path)
            if sha256 in seen_hashes:
                continue
            seen_hashes.add(sha256)
            faces.extend(inspect_font(path, origins[path]))
        except Exception as error:
            rejected.append({"path": str(path), "reason": "font_unusable",
                             "detail": type(error).__name__})

    families: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for face in faces:
        families[face["family"]].append(face)
    ranked = []
    for family in sorted(families):
        scores, missing = [], []
        for sample in samples:
            alternatives = []
            for face in families[family]:
                if not matches_variant(face, sample["variants"]):
                    continue
                try:
                    rendered = render_text(sample["text"], face)
                    rendered_geometry = ink_metrics(rendered)
                    observed = observed_geometry[sample["id"]]
                    if observed is None or rendered_geometry is None:
                        continue
                    score = similarity(target_inks[sample["id"]], normalized_ink(rendered))
                    geometry = estimate_geometry(observed, rendered_geometry, sample["text"],
                                                 pt_per_px)
                    alternatives.append((score, face, geometry))
                except Exception:
                    continue
            if not alternatives:
                missing.append(sample["id"])
                continue
            score, face, geometry = max(alternatives,
                                        key=lambda item: (item[0], item[1]["style"], item[1]["path"]))
            scores.append({"id": sample["id"], "text": sample["text"], "weight": sample["weight"],
                           "dimensions": sample["dimensions"], "similarity": round(score, 6),
                           "style": face["style"], "variant": face["variants"][0],
                           "font_path": face["path"], "face_index": face["face_index"],
                           "sha256": face["sha256"], "geometry": geometry})
        required_missing = [sample["id"] for sample in samples
                            if sample["required"] and sample["id"] in missing]
        if required_missing:
            rejected.append({"family": family, "reason": "required_evidence_missing",
                             "samples": required_missing})
            continue
        representative = max(scores, key=lambda item: (item["similarity"] * item["weight"], item["style"]))
        face = next(item for item in families[family]
                    if item["path"] == representative["font_path"]
                    and item["face_index"] == representative["face_index"])
        ranked.append({"family": family, "style": face["style"], "font_path": face["path"],
                       "face_index": face["face_index"], "sha256": face["sha256"],
                       "origin": face["origin"], "license": face["license"],
                       "license_url": face["license_url"], "copyright": face["copyright"],
                       "units_per_em": face["units_per_em"],
                       "m_advance_em": face["m_advance_em"],
                       "overall_score": weighted_score(scores),
                       "family_score": weighted_score(scores, "family"),
                       "weight_score": weighted_score(scores, "weight"),
                       "style_score": weighted_score(scores, "style"),
                       "evidence": scores, "optional_missing": missing})
    ranked.sort(key=lambda item: (-item["overall_score"], item["family"], item["style"],
                                  item["font_path"]))
    ranked = ranked[:limit]
    if not ranked:
        status = "unknown"
    elif ranked[0]["overall_score"] >= threshold:
        status = "matched"
    else:
        status = "fallback_required"
    return {"schema_version": 1, "status": status, "network_access": "none",
            "query": {"evidence": str(evidence_path.resolve()), "threshold": threshold,
                      "limit": limit, "directories": effective_dirs, "pt_per_px": pt_per_px},
            "ranked": ranked, "rejected": sorted(rejected,
                                                   key=lambda item: (item.get("family", ""),
                                                                     item.get("path", ""),
                                                                     item["reason"]))}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--font-dir", type=Path, action="append", default=[])
    parser.add_argument("--threshold", type=float, default=0.70)
    parser.add_argument("--limit", type=int, default=30)
    parser.add_argument("--pt-per-px", type=float)
    parser.add_argument("--no-fontconfig", action="store_true")
    args = parser.parse_args()
    try:
        result = discover(args.evidence, args.font_dir, args.threshold, args.limit,
                          not args.no_fontconfig, args.pt_per_px)
        json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"local-font-discovery: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
