#!/usr/bin/env python3
"""Execute the controlled difficult-font corpus through real PDF tooling."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import candidate_raster_profile
import scan_word_compare


KINDS = ("serif", "sans", "mono")


def catalog(binary: Path, pdf: Path) -> dict[str, Any]:
    process = subprocess.run(
        [str(binary), str(pdf), "--page", "0"], capture_output=True, text=True,
        check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(process.stderr.strip() or "font catalog failed")
    return json.loads(process.stdout)


def compile_fixture(source: Path, output: Path) -> None:
    process = subprocess.run(
        ["typst", "compile", str(source), str(output)], capture_output=True,
        text=True, check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(process.stderr.strip() or "typst compilation failed")


def run(binary: Path, fixture_directory: Path, width: int = 1276, height: int = 425) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="decalque-font-corpus-") as directory:
        temporary = Path(directory)
        catalogs, profiles = {}, {}
        for kind in KINDS:
            pdf = temporary / f"{kind}.pdf"
            compile_fixture(fixture_directory / f"typography-{kind}.typ", pdf)
            catalogs[kind] = catalog(binary, pdf)
            image = candidate_raster_profile.render_page(pdf, 0, width, height)
            profiles[kind] = candidate_raster_profile.candidate_profiles(
                catalogs[kind], image, width, height
            )

        reference_words = scan_word_compare.candidate_words(catalogs["serif"]["glyphs"])
        cases = []
        for kind in KINDS:
            segments = []
            for word in reference_words:
                word_key = scan_word_compare.key(word["text"])
                segments.append({
                    "text": word["text"],
                    "bbox_pt": [word["x0"], word["baseline_y"] - word["font_size_pt"], word["x1"], word["baseline_y"]],
                    "baseline_y_pt": word["baseline_y"], "baseline_confidence": 1.0,
                    "typographic_profile": profiles["serif"][word_key][0],
                    "candidate_typographic_profile": profiles[kind][word_key][0],
                })
            page = {
                "point_transform": {
                    "scale_y_pt_per_px": catalogs["serif"]["page"]["height_pt"] / height
                },
                "regions": [{"detected_lines": [{"id": 0, "word_segments": segments}]}],
            }
            comparison = scan_word_compare.compare(page, catalogs[kind])
            statuses = [word["typography_status"] for word in comparison["words"]]
            detected = "violated" in statuses
            cases.append({
                "candidate": kind,
                "expected": "preserved" if kind == "serif" else "violated",
                "status": "violated" if detected else (
                    "preserved" if statuses and all(status == "preserved" for status in statuses)
                    else "unknown"
                ),
                "words": [
                    {
                        "text": word["word"],
                        "status": word["typography_status"],
                        "shape_distance": word["typographic_shape_distance"],
                    }
                    for word in comparison["words"]
                ],
            })
        mutations = [case for case in cases if case["candidate"] != "serif"]
        rejected = sum(case["status"] == "violated" for case in mutations)
        return {
            "schema_version": 1,
            "reference": "serif",
            "cases": cases,
            "mutation_score": rejected / len(mutations),
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog-bin", type=Path, default=Path("target/debug/decalque-font-catalog"))
    parser.add_argument("--fixtures", type=Path, default=Path(__file__).parent / "tests" / "fixtures")
    args = parser.parse_args()
    try:
        json.dump(run(args.catalog_bin, args.fixtures), sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"typography-corpus: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
