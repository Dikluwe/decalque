#!/usr/bin/env python3
"""Exercise multiline layout mutations through real PDF tooling."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

import scan_word_compare


CASES = (
    ("multiline", "preserved"),
    ("leading", "violated"),
    ("reflow", "violated"),
)


def compile_fixture(source: Path, output: Path) -> None:
    process = subprocess.run(
        ["typst", "compile", str(source), str(output)], capture_output=True,
        text=True, check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(process.stderr.strip() or "typst compilation failed")


def catalog(binary: Path, pdf: Path) -> dict[str, Any]:
    process = subprocess.run(
        [str(binary), str(pdf), "--page", "0"], capture_output=True, text=True,
        check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(process.stderr.strip() or "font catalog failed")
    return json.loads(process.stdout)


def observed_page(reference: dict[str, Any]) -> dict[str, Any]:
    lines: dict[int, list[dict[str, Any]]] = {}
    for word in scan_word_compare.candidate_words(reference["glyphs"]):
        lines.setdefault(word["line_id"], []).append({
            "text": word["text"],
            "bbox_pt": [
                word["x0"], word["baseline_y"] - word["font_size_pt"],
                word["x1"], word["baseline_y"],
            ],
            "baseline_y_pt": word["baseline_y"],
            "baseline_confidence": 1.0,
        })
    return {
        "regions": [{"detected_lines": [
            {"id": line_id, "word_segments": words}
            for line_id, words in sorted(lines.items())
        ]}],
    }


def run(binary: Path, fixture_directory: Path) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix="decalque-layout-corpus-") as directory:
        temporary = Path(directory)
        catalogs = {}
        for name, _ in CASES:
            pdf = temporary / f"{name}.pdf"
            compile_fixture(fixture_directory / f"layout-{name}.typ", pdf)
            catalogs[name] = catalog(binary, pdf)

        page = observed_page(catalogs["multiline"])
        cases = []
        for name, expected in CASES:
            comparison = scan_word_compare.compare(page, catalogs[name])
            word_statuses = [word["status"] for word in comparison["words"]]
            line_statuses = [line["status"] for line in comparison["lines"]]
            statuses = word_statuses + line_statuses
            status = (
                "violated" if "violated" in statuses else
                "preserved" if statuses and all(item == "preserved" for item in statuses)
                else "unknown"
            )
            cases.append({
                "candidate": name,
                "expected": expected,
                "status": status,
                "words": comparison["words"],
                "lines": comparison["lines"],
            })

        mutations = [case for case in cases if case["expected"] == "violated"]
        return {
            "schema_version": 1,
            "reference": "multiline",
            "mutation_score": (
                sum(case["status"] == "violated" for case in mutations) / len(mutations)
            ),
            "cases": cases,
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--catalog-bin", type=Path,
        default=Path("target/debug/decalque-font-catalog"),
    )
    parser.add_argument(
        "--fixtures", type=Path,
        default=Path(__file__).parent / "tests" / "fixtures",
    )
    args = parser.parse_args()
    try:
        json.dump(run(args.catalog_bin, args.fixtures), sys.stdout, ensure_ascii=False,
                  separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"layout-corpus: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
