#!/usr/bin/env python3
"""Exercise OCR text errors through the scan comparison process boundary."""

from __future__ import annotations

import argparse
import copy
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import layout_corpus


def compare_process(script: Path, page: dict[str, Any], catalog: dict[str, Any], directory: Path) -> dict[str, Any]:
    scan_path = directory / "scan.json"
    catalog_path = directory / "catalog.json"
    scan_path.write_text(json.dumps([page]), encoding="utf-8")
    catalog_path.write_text(json.dumps(catalog), encoding="utf-8")
    process = subprocess.run(
        [sys.executable, str(script), str(scan_path), str(catalog_path)],
        capture_output=True, text=True, check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(process.stderr.strip() or "scan comparison failed")
    return json.loads(process.stdout)


def mutated_pages(reference: dict[str, Any]) -> list[tuple[str, str, dict[str, Any]]]:
    exact = layout_corpus.observed_page(reference)
    substituted = copy.deepcopy(exact)
    substituted["regions"][0]["detected_lines"][0]["word_segments"][0]["text"] = "minimurn"

    inserted = copy.deepcopy(exact)
    inserted["regions"][0]["detected_lines"][0]["word_segments"].append({
        "text": "phantom", "bbox_pt": [360, 29, 430, 49],
        "baseline_y_pt": 49, "baseline_confidence": 1.0,
    })

    omitted = copy.deepcopy(exact)
    omitted["regions"][0]["detected_lines"][0]["word_segments"].pop(2)

    merged = copy.deepcopy(exact)
    words = merged["regions"][0]["detected_lines"][0]["word_segments"]
    first, second = words[:2]
    words[:2] = [{
        "text": first["text"] + second["text"],
        "bbox_pt": [first["bbox_pt"][0], first["bbox_pt"][1], second["bbox_pt"][2], second["bbox_pt"][3]],
        "baseline_y_pt": first["baseline_y_pt"], "baseline_confidence": 1.0,
    }]
    return [
        ("exact", "preserved", exact),
        ("character-substitution", "detected", substituted),
        ("inserted-word", "detected", inserted),
        ("omitted-word", "detected", omitted),
        ("merged-space", "detected", merged),
    ]


def run(binary: Path, fixture: Path, compare_script: Path) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="decalque-ocr-errors-") as directory_name:
        directory = Path(directory_name)
        pdf = directory / "reference.pdf"
        layout_corpus.compile_fixture(fixture, pdf)
        reference = layout_corpus.catalog(binary, pdf)
        cases = []
        for name, expected, page in mutated_pages(reference):
            report = compare_process(compare_script, page, reference, directory)
            unknown = report["counts"]["unknown"]
            unmatched = len(report["unmatched_candidate_words"])
            status = "detected" if unknown or unmatched else "preserved"
            cases.append({
                "case": name, "expected": expected, "status": status,
                "unknown_scan_words": unknown,
                "unmatched_candidate_words": report["unmatched_candidate_words"],
                "coverage": report["coverage"],
            })
        mutations = cases[1:]
        return {
            "schema_version": 1,
            "mutation_score": sum(case["status"] == "detected" for case in mutations) / len(mutations),
            "cases": cases,
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--catalog-bin", type=Path, default=Path("target/debug/decalque-font-catalog"))
    parser.add_argument("--fixture", type=Path, default=Path(__file__).parent / "tests" / "fixtures" / "layout-multiline.typ")
    parser.add_argument("--compare", type=Path, default=Path(__file__).parent / "scan_word_compare.py")
    args = parser.parse_args()
    try:
        json.dump(run(args.catalog_bin, args.fixture, args.compare), sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"ocr-error-corpus: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
