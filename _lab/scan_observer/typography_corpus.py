#!/usr/bin/env python3
"""Execute the controlled difficult-font corpus through real PDF tooling."""

from __future__ import annotations

import argparse
import io
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import candidate_raster_profile
import scan_word_compare


KINDS = (
    "serif", "vera-serif", "liberation-serif", "sans", "mono", "bold",
    "italic", "condensed", "size-minus-half", "size-plus-half", "tracking",
    "word-spacing", "baseline-shift",
)

EXPECTED = {
    "serif": "preserved",
    # DejaVu Serif extends Bitstream Vera Serif; this Latin sample has identical ink.
    "vera-serif": "preserved",
}


def degraded_images(image: Any) -> dict[str, Any]:
    import numpy as np
    from PIL import Image, ImageFilter

    rgb = Image.fromarray(image[:, :, ::-1])
    width, height = rgb.size
    low_resolution = rgb.resize(
        (width // 2, height // 2), Image.Resampling.LANCZOS
    ).resize((width, height), Image.Resampling.BILINEAR)
    blurred = rgb.filter(ImageFilter.GaussianBlur(radius=0.8))
    jpeg_buffer = io.BytesIO()
    rgb.save(jpeg_buffer, format="JPEG", quality=45)
    jpeg_buffer.seek(0)
    jpeg = Image.open(jpeg_buffer).convert("RGB")
    rotated = rgb.rotate(
        0.35, resample=Image.Resampling.BICUBIC, expand=False, fillcolor="white"
    )
    generator = np.random.default_rng(20260914)
    noisy = np.clip(
        np.asarray(rgb, dtype=np.int16)
        + generator.normal(0, 4, (height, width, 1)).round().astype(np.int16),
        0, 255,
    ).astype(np.uint8)
    return {
        "pristine": image,
        "low-resolution": np.asarray(low_resolution)[:, :, ::-1].copy(),
        "blur": np.asarray(blurred)[:, :, ::-1].copy(),
        "noise": noisy[:, :, ::-1].copy(),
        "jpeg": np.asarray(jpeg)[:, :, ::-1].copy(),
        "rotation-0.35deg": np.asarray(rotated)[:, :, ::-1].copy(),
    }


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
        catalogs, images, profiles = {}, {}, {}
        for kind in KINDS:
            pdf = temporary / f"{kind}.pdf"
            compile_fixture(fixture_directory / f"typography-{kind}.typ", pdf)
            catalogs[kind] = catalog(binary, pdf)
            image = candidate_raster_profile.render_page(pdf, 0, width, height)
            images[kind] = image
            profiles[kind] = candidate_raster_profile.candidate_profiles(
                catalogs[kind], image, width, height
            )

        reference_words = scan_word_compare.candidate_words(catalogs["serif"]["glyphs"])

        def evaluate(observed_profiles: dict[str, list[dict[str, Any]]]) -> list[dict[str, Any]]:
            cases = []
            for kind in KINDS:
                segments = []
                for word in reference_words:
                    word_key = scan_word_compare.key(word["text"])
                    segments.append({
                        "text": word["text"],
                        "bbox_pt": [word["x0"], word["baseline_y"] - word["font_size_pt"], word["x1"], word["baseline_y"]],
                        "baseline_y_pt": word["baseline_y"], "baseline_confidence": 1.0,
                        "typographic_profile": observed_profiles[word_key][0],
                        "candidate_typographic_profile": profiles[kind][word_key][0],
                    })
                page = {
                    "point_transform": {
                        "scale_y_pt_per_px": catalogs["serif"]["page"]["height_pt"] / height
                    },
                    "regions": [{"detected_lines": [{"id": 0, "word_segments": segments}]}],
                }
                comparison = scan_word_compare.compare(page, catalogs[kind])
                statuses = [
                    "violated"
                    if "violated" in (word["status"], word["typography_status"])
                    else (
                        "preserved"
                        if word["status"] == word["typography_status"] == "preserved"
                        else "unknown"
                    )
                    for word in comparison["words"]
                ]
                detected = "violated" in statuses
                cases.append({
                    "candidate": kind,
                    "expected": EXPECTED.get(kind, "violated"),
                    "status": "violated" if detected else (
                        "preserved" if statuses and all(status == "preserved" for status in statuses)
                        else "unknown"
                    ),
                    "words": [
                        {
                            "text": word["word"],
                            "status": status,
                            "geometry_status": word["status"],
                            "typography_status": word["typography_status"],
                            "shape_distance": word["typographic_shape_distance"],
                        }
                        for word, status in zip(comparison["words"], statuses)
                    ],
                })
            return cases

        observations = []
        for degradation, observed_image in degraded_images(images["serif"]).items():
            observed_profiles = candidate_raster_profile.candidate_profiles(
                catalogs["serif"], observed_image, width, height
            )
            cases = evaluate(observed_profiles)
            mutations = [case for case in cases if case["expected"] == "violated"]
            rejected = sum(case["status"] == "violated" for case in mutations)
            observations.append({
                "degradation": degradation,
                "control_status": cases[0]["status"],
                "mutation_score": rejected / len(mutations),
                "cases": cases,
            })

        pristine = observations[0]
        return {
            "schema_version": 2,
            "reference": "serif",
            "cases": pristine["cases"],
            "mutation_score": pristine["mutation_score"],
            "degradations": observations,
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
