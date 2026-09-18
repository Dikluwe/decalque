#!/usr/bin/env python3
# Crystalline Lineage
# @prompt 00_nucleo/prompts/zone-typst-materialization.md
# @layer L5
# @updated 2026-09-16
"""Materialize marked zones as frozen native Typst lines and raster visual regions."""

from __future__ import annotations

import argparse
import json
import math
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

from PIL import Image, ImageChops, ImageFont, ImageStat

import typst_page_materializer


def command(args: list[str], label: str) -> str:
    process = subprocess.run(args, capture_output=True, text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or f"{label} falhou")
    return process.stdout


def build_manifest(zones: dict[str, Any], image_path: Path, font_path: Path,
                   output_dir: Path, width_pt: float, height_pt: float) -> dict[str, Any]:
    if not font_path.is_file():
        raise ValueError(f"fonte não encontrada: {font_path}")
    with Image.open(image_path) as image:
        width_px, height_px = image.size
        visual_dir = output_dir / "visuals"
        visual_dir.mkdir(parents=True, exist_ok=True)
        regions = []
        visual_index = 0
        family = ImageFont.truetype(str(font_path), 20).getname()[0]
        sy = height_pt / height_px
        for zone in zones.get("regions", []):
            editorial_role = zone.get("editorial_role", {}).get("role")
            if zone.get("type") == "visual" or editorial_role == "visual":
                box = [int(round(value)) for value in zone["bbox"]]
                visual_index += 1
                target = visual_dir / f"visual-{visual_index:03d}.png"
                image.crop(tuple(box)).save(target)
                regions.append({"kind": "image", "bbox_px": box,
                                "path": str(target.resolve())})
                continue
            style_summary = zone.get("font_style_summary", {})
            italic = (style_summary.get("slant") == "italic"
                      or editorial_role in {"verse_or_title_list", "block_quote",
                                            "biographical_note"}
                      or (not editorial_role
                          and zone.get("type") in {"list", "note", "epigraph"}))
            weight = style_summary.get("weight", "regular")
            if weight not in {"light", "regular", "bold"}:
                weight = "regular"
            for line in zone.get("lines", []):
                text = str(line.get("text", "")).strip()
                box = [float(value) for value in line.get("bbox", [])]
                if not text or len(box) != 4 or box[2] <= box[0] or box[3] <= box[1]:
                    raise ValueError("linha textual inválida")
                size_px = max(6.0, (box[3] - box[1]) * 0.96)
                pil_font = ImageFont.truetype(str(font_path), max(1, round(size_px)))
                natural = float(pil_font.getlength(text))
                scale_x = (box[2] - box[0]) / natural if natural else 1.0
                regions.append({
                    "kind": "text", "bbox_px": box, "text": text,
                    "editorial_role": editorial_role or zone.get("type", "unknown"),
                    "scale_x": scale_x, "single_line": True,
                    "font": {"family": family, "size_pt": size_px * sy,
                             "weight": weight, "style": "italic" if italic else "normal",
                             "tracking_em": 0.0, "leading_pt": size_px * sy,
                             "skew_deg": -10 if italic else 0},
                })
    return {"schema_version": 1, "page": {"width_pt": width_pt, "height_pt": height_pt,
            "source_width_px": width_px, "source_height_px": height_px}, "regions": regions}


def run(zones_path: Path, image_path: Path, font_path: Path, output_dir: Path,
        width_pt: float, height_pt: float, dpi: int) -> dict[str, Any]:
    zones = json.loads(zones_path.read_text(encoding="utf-8"))
    output_dir.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(zones, image_path, font_path, output_dir, width_pt, height_pt)
    manifest_path = output_dir / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
    typst_path, pdf_path = output_dir / "page.typ", output_dir / "page.pdf"
    typst_path.write_text(typst_page_materializer.materialize(manifest, manifest_path))
    command(["typst", "compile", "--root", "/", "--font-path", str(font_path.parent),
             str(typst_path), str(pdf_path)], "compilação Typst")
    raster_prefix = output_dir / "page"
    command(["pdftoppm", "-f", "1", "-l", "1", "-singlefile", "-r", str(dpi),
             "-png", str(pdf_path), str(raster_prefix)], "rasterização")
    rendered_path = raster_prefix.with_suffix(".png")
    extracted = command(["pdftotext", "-layout", str(pdf_path), "-"], "extração textual")
    normalized_extracted = re.sub(r"\s+", " ", extracted).strip()
    missing = [line["text"] for region in zones["regions"] for line in region.get("lines", [])
               if re.sub(r"\s+", " ", line["text"]).strip() not in normalized_extracted]
    if missing:
        raise ValueError(f"PDF não preservou {len(missing)} linha(s) textuais")
    with Image.open(image_path).convert("RGB") as original, Image.open(rendered_path).convert("RGB") as rendered:
        reference = original.resize(rendered.size, Image.Resampling.LANCZOS)
        difference = ImageChops.difference(reference, rendered)
        difference.save(output_dir / "comparison.png")
        mae = sum(ImageStat.Stat(difference).mean) / (3 * 255)
        rendered_size = list(rendered.size)
    report = {"schema_version": 1, "status": "success", "line_count": sum(
              len(region.get("lines", [])) for region in zones["regions"]),
              "visual_count": sum(region.get("type") == "visual" for region in zones["regions"]),
              "page_size_px": rendered_size, "mean_absolute_error": round(mae, 6),
              "pdf": str(pdf_path.resolve()), "rendered": str(rendered_path.resolve()),
              "comparison": str((output_dir / "comparison.png").resolve()),
              "font": str(font_path.resolve()),
              "limitations": ["visual-regions-remain-raster", "italic-is-synthetic-skew"]}
    (output_dir / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("zones", type=Path)
    parser.add_argument("image", type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--width-pt", type=float, default=452.16)
    parser.add_argument("--height-pt", type=float, default=714.24)
    parser.add_argument("--dpi", type=int, default=180)
    args = parser.parse_args()
    try:
        print(json.dumps(run(args.zones, args.image, args.font, args.output_dir,
                             args.width_pt, args.height_pt, args.dpi), ensure_ascii=False, indent=2))
        return 0
    except Exception as error:
        print(f"zone-typst-reconstructor: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
