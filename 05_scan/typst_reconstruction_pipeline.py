#!/usr/bin/env python3
"""Generate, compile, rasterize and visually measure one reconstructed page."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image

import typst_page_materializer


def command(arguments: list[str], label: str) -> None:
    process = subprocess.run(arguments, capture_output=True, text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or f"{label} falhou")


def compare_images(reference: Path, candidate: Path, difference_path: Path) -> dict:
    with Image.open(reference) as left_source, Image.open(candidate) as right_source:
        left = np.asarray(left_source.convert("L"), dtype=np.int16)
        right = np.asarray(right_source.convert("L"), dtype=np.int16)
    if left.shape != right.shape:
        return {"status": "not_comparable", "reference_size": list(left.shape[::-1]),
                "candidate_size": list(right.shape[::-1]), "mean_absolute_error": None,
                "different_pixel_ratio": None}
    difference = np.abs(left - right)
    left_ink = 1.0 - left / 255.0
    right_ink = 1.0 - right / 255.0
    horizontal_error = float(np.abs(left_ink.mean(axis=1) - right_ink.mean(axis=1)).mean())
    vertical_error = float(np.abs(left_ink.mean(axis=0) - right_ink.mean(axis=0)).mean())
    Image.fromarray(np.minimum(difference * 4, 255).astype(np.uint8), mode="L").save(difference_path)
    return {"status": "comparable", "reference_size": list(left.shape[::-1]),
            "candidate_size": list(right.shape[::-1]),
            "mean_absolute_error": round(float(difference.mean() / 255), 6),
            "different_pixel_ratio": round(float((difference > 16).mean()), 6),
            "horizontal_line_error": round(horizontal_error, 6),
            "vertical_line_error": round(vertical_error, 6)}


def run(manifest_path: Path, source_pdf: Path, output_dir: Path, source_page: int, dpi: int) -> dict:
    output_dir.mkdir(parents=True, exist_ok=True)
    typ_path = output_dir / "reconstruction.typ"
    pdf_path = output_dir / "reconstruction.pdf"
    reference_prefix = output_dir / "reference"
    candidate_prefix = output_dir / "candidate"
    manifest = typst_page_materializer.load_manifest(manifest_path)
    typ_path.write_text(typst_page_materializer.materialize(manifest, manifest_path))
    command(["typst", "compile", "--root", "/", str(typ_path), str(pdf_path)], "compilação Typst")
    command(["pdftoppm", "-f", str(source_page), "-l", str(source_page), "-r", str(dpi),
             "-gray", "-png", "-singlefile", str(source_pdf), str(reference_prefix)],
            "rasterização da referência")
    command(["pdftoppm", "-f", "1", "-l", "1", "-r", str(dpi), "-gray", "-png",
             "-singlefile", str(pdf_path), str(candidate_prefix)], "rasterização do candidato")
    reference_png = reference_prefix.with_suffix(".png")
    candidate_png = candidate_prefix.with_suffix(".png")
    difference_png = output_dir / "difference.png"
    return {"schema_version": 1, "manifest": str(manifest_path.resolve()),
            "typst": str(typ_path.resolve()), "pdf": str(pdf_path.resolve()),
            "reference_png": str(reference_png.resolve()), "candidate_png": str(candidate_png.resolve()),
            "difference_png": str(difference_png.resolve()),
            "visual_comparison": compare_images(reference_png, candidate_png, difference_png)}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("source_pdf", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--source-page", type=int, default=1)
    parser.add_argument("--dpi", type=int, default=180)
    args = parser.parse_args()
    try:
        result = run(args.manifest, args.source_pdf, args.output_dir, args.source_page, args.dpi)
        json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"typst-reconstruction-pipeline: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
