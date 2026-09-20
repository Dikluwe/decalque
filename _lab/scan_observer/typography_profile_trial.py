#!/usr/bin/env python3
"""Render sample pages, normalize their frame, freeze metrics and create an audit PDF."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageDraw

import frozen_typography_profile
import page_geometry_normalizer


def _command(args: list[str], label: str) -> None:
    process = subprocess.run(args, capture_output=True, text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or f"{label} falhou")


def _page_size(pdf: Path, page: int) -> tuple[float, float]:
    process = subprocess.run(["pdfinfo", "-f", str(page), "-l", str(page), str(pdf)],
                             capture_output=True, text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or "pdfinfo falhou")
    match = re.search(r"Page(?:\s+\d+)? size:\s+([0-9.]+) x ([0-9.]+) pts", process.stdout)
    if not match:
        raise ValueError(f"tamanho físico ausente para página {page}")
    return float(match.group(1)), float(match.group(2))


def run(pdf: Path, output_dir: Path, pages: list[int], family: str,
        width_pt: float | None, height_pt: float | None, dpi: int) -> dict:
    output_dir.mkdir(parents=True, exist_ok=True)
    normalized_dir = output_dir / "normalized"
    normalized_dir.mkdir(exist_ok=True)
    observations, geometry_reports, normalized_images = [], [], []
    for page in pages:
        prefix = output_dir / f"source-{page:03d}"
        _command(["pdftoppm", "-f", str(page), "-l", str(page), "-singlefile",
                  "-r", str(dpi), "-png", str(pdf), str(prefix)], "rasterização")
        source_path = prefix.with_suffix(".png")
        with Image.open(source_path) as source:
            normalized, geometry = page_geometry_normalizer.normalize_image(source.convert("RGB"))
        normalized_path = normalized_dir / f"page-{page:03d}.png"
        normalized.save(normalized_path)
        observed_width_pt, observed_height_pt = ((width_pt, height_pt)
                                                  if width_pt and height_pt
                                                  else _page_size(pdf, page))
        observation = frozen_typography_profile.observe_geometry(
            normalized, page, observed_width_pt, observed_height_pt)
        observations.append(observation)
        geometry_reports.append({"page": page, **geometry})
        normalized_images.append(normalized)
    profile = frozen_typography_profile.freeze_profile(observations, family)
    audits = [frozen_typography_profile.audit_observation(profile, item)
              for item in observations]
    annotated = []
    for image, observation, audit, geometry in zip(
            normalized_images, observations, audits, geometry_reports):
        canvas = image.copy()
        draw = ImageDraw.Draw(canvas)
        color = "#16803a" if audit["status"] == "conformant" else "#b55b00"
        for index, box in enumerate(observation["columns"], 1):
            draw.rectangle(tuple(box), outline=color, width=3)
            draw.text((box[0] + 5, box[1] + 5), f"column {index}", fill=color,
                      stroke_width=1, stroke_fill="white")
        zone_colors = {"visual": "#a03da0", "heading": "#1b61a8", "rule": "#777777"}
        for zone in observation.get("spanning_zones", []):
            zone_color = zone_colors.get(zone["kind"], "#b55b00")
            draw.rectangle(tuple(zone["bbox"]), outline=zone_color, width=3)
            draw.text((zone["bbox"][0] + 5, zone["bbox"][1] + 5), zone["kind"],
                      fill=zone_color, stroke_width=1, stroke_fill="white")
        label = (f"page {observation['page']} | skew {geometry.get('observed_skew_deg')} deg | "
                 f"{audit['status']} | frozen {profile['profile_hash'][:10]}")
        draw.rectangle((0, 0, canvas.width, 28), fill="white")
        draw.text((8, 7), label, fill="black")
        annotated.append(canvas.convert("RGB"))
    audit_pdf = output_dir / "typography-audit.pdf"
    annotated[0].save(audit_pdf, "PDF", resolution=dpi, save_all=True,
                      append_images=annotated[1:])
    report = {"schema_version": 1, "status": "success", "source": str(pdf.resolve()),
              "pages": pages, "profile": profile, "geometry": geometry_reports,
              "observations": observations, "audits": audits,
              "audit_pdf": str(audit_pdf.resolve()),
              "limitations": ["text-recognition-not-required-for-metrics",
                              "perspective-remains-unknown-without-four-borders"]}
    (output_dir / "profile.json").write_text(
        json.dumps(profile, ensure_ascii=False, indent=2) + "\n")
    (output_dir / "report.json").write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("pdf", type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--pages", default="10,11,12,13,14")
    parser.add_argument("--family", default="Nimbus Roman")
    parser.add_argument("--width-pt", type=float)
    parser.add_argument("--height-pt", type=float)
    parser.add_argument("--dpi", type=int, default=120)
    args = parser.parse_args()
    try:
        pages = [int(value) for value in args.pages.split(",")]
        result = run(args.pdf, args.output_dir, pages, args.family,
                     args.width_pt, args.height_pt, args.dpi)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except Exception as error:
        print(f"typography-profile-trial: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
