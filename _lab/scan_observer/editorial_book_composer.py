#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/editorial-typst-composition.md
"""Resumable, timed whole-book execution for editorial Typst composition."""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import re
import subprocess
import sys
import time
from collections import defaultdict
from pathlib import Path
from typing import Any

import editorial_typst_composer


PAGE_PATTERN = re.compile(r"^page-(\d+)\.(?:json|png)$")


def _numbered(directory: Path, suffix: str, start: int, end: int | None) -> dict[int, Path]:
    result = {}
    for path in directory.glob(f"page-*.{suffix}"):
        match = PAGE_PATTERN.match(path.name)
        if not match:
            continue
        number = int(match.group(1))
        if number >= start and (end is None or number <= end):
            result[number] = path
    return result


def discover_pages(classified_dir: Path, image_dir: Path, start: int = 1,
                   end: int | None = None) -> dict[str, Any]:
    classified = _numbered(classified_dir, "json", start, end)
    images = _numbered(image_dir, "png", start, end)
    common = sorted(set(classified) & set(images))
    return {"ready": [(number, classified[number], images[number]) for number in common],
            "missing_images": sorted(set(classified) - set(images)),
            "missing_classified": sorted(set(images) - set(classified))}


def _complete(page_dir: Path) -> bool:
    report_path = page_dir / "report.json"
    pdf_path = page_dir / "page.pdf"
    if not report_path.is_file() or not pdf_path.is_file():
        return False
    try:
        return json.loads(report_path.read_text()).get("status") in {"success", "partial"}
    except (OSError, ValueError):
        return False


def process_pages(pages: list[tuple[int, Path, Path]], output_dir: Path, font_path: Path,
                  width_pt: float, height_pt: float, dpi: int, workers: int,
                  resume: bool) -> dict[str, Any]:
    pages_dir = output_dir / "pages"
    pages_dir.mkdir(parents=True, exist_ok=True)
    skipped, pending = [], []
    for item in pages:
        page_dir = pages_dir / f"page-{item[0]:03d}"
        if resume and _complete(page_dir):
            skipped.append(item[0])
        else:
            pending.append(item)
    processed, partial, failures, reports = [], [], [], {}

    def execute(item: tuple[int, Path, Path]) -> tuple[int, dict[str, Any]]:
        number, classified, image = item
        target = pages_dir / f"page-{number:03d}"
        report = editorial_typst_composer.compose(
            classified, image, target, font_path, width_pt, height_pt, number, dpi)
        return number, report

    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, workers)) as executor:
        futures = {executor.submit(execute, item): item[0] for item in pending}
        for future in concurrent.futures.as_completed(futures):
            number = futures[future]
            try:
                result_number, report = future.result()
                reports[result_number] = report
                if report.get("status") in {"success", "partial"}:
                    processed.append(result_number)
                    if report.get("status") == "partial":
                        partial.append(result_number)
                else:
                    failures.append({"page": result_number, "error": report.get("status", "unknown")})
            except Exception as error:
                failures.append({"page": number, "error": str(error)})
    totals: defaultdict[str, float] = defaultdict(float)
    for report in reports.values():
        for key, value in report.get("timings_seconds", {}).items():
            totals[key] += float(value)
    return {"processed_pages": sorted(processed), "skipped_pages": sorted(skipped),
            "partial_pages": sorted(partial),
            "failed_pages": sorted(failures, key=lambda item: item["page"]),
            "timings_seconds": {key: round(value, 6) for key, value in sorted(totals.items())},
            "page_reports": reports}


def _merge(pages: list[int], pages_dir: Path, target: Path) -> float:
    inputs = [pages_dir / f"page-{number:03d}" / "page.pdf" for number in pages]
    missing = [str(path) for path in inputs if not path.is_file()]
    if missing:
        raise ValueError(f"PDFs ausentes para união: {len(missing)}")
    started = time.perf_counter()
    process = subprocess.run(["pdfunite", *map(str, inputs), str(target)],
                             capture_output=True, text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or "pdfunite falhou")
    return time.perf_counter() - started


def run(classified_dir: Path, image_dir: Path, output_dir: Path, font_path: Path,
        start: int, end: int | None, workers: int, resume: bool, width_pt: float,
        height_pt: float, dpi: int, merge: bool) -> dict[str, Any]:
    wall_started = time.perf_counter()
    discovery = discover_pages(classified_dir, image_dir, start, end)
    if not discovery["ready"]:
        raise ValueError("nenhuma página correspondente encontrada")
    result = process_pages(discovery["ready"], output_dir, font_path, width_pt, height_pt,
                           dpi, workers, resume)
    ready_numbers = [item[0] for item in discovery["ready"]]
    merge_seconds = None
    book_pdf = None
    if merge and not result["failed_pages"]:
        output_dir.mkdir(parents=True, exist_ok=True)
        book_pdf = output_dir / "book.pdf"
        merge_seconds = _merge(ready_numbers, output_dir / "pages", book_pdf)
    all_partial_pages = []
    all_stage_totals: defaultdict[str, float] = defaultdict(float)
    for number in ready_numbers:
        report_path = output_dir / "pages" / f"page-{number:03d}" / "report.json"
        if not report_path.is_file():
            continue
        page_report = json.loads(report_path.read_text())
        if page_report.get("status") == "partial":
            all_partial_pages.append(number)
        for key, value in page_report.get("timings_seconds", {}).items():
            all_stage_totals[key] += float(value)
    aggregate = {
        "schema_version": 1,
        "status": "success" if not result["failed_pages"] and not discovery["missing_images"]
                  and not discovery["missing_classified"] else "partial",
        "requested_range": {"start": start, "end": end},
        "ready_page_count": len(ready_numbers),
        "processed_page_count": len(result["processed_pages"]),
        "skipped_page_count": len(result["skipped_pages"]),
        "text_review_pages": all_partial_pages,
        "failed_pages": result["failed_pages"],
        "missing_images": discovery["missing_images"],
        "missing_classified": discovery["missing_classified"],
        "workers": workers, "dpi": dpi,
        "summed_stage_seconds": {key: round(value, 6)
                                  for key, value in sorted(all_stage_totals.items())},
        "merge_seconds": round(merge_seconds, 6) if merge_seconds is not None else None,
        "wall_seconds": round(time.perf_counter() - wall_started, 6),
        "book_pdf": str(book_pdf.resolve()) if book_pdf else None,
    }
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "book-report.json").write_text(
        json.dumps(aggregate, ensure_ascii=False, indent=2) + "\n")
    return aggregate


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("classified_dir", type=Path)
    parser.add_argument("image_dir", type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--start", type=int, default=1)
    parser.add_argument("--end", type=int)
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument("--resume", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--merge", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--width-pt", type=float, default=452.16)
    parser.add_argument("--height-pt", type=float, default=714.24)
    parser.add_argument("--dpi", type=int, default=120)
    args = parser.parse_args()
    try:
        result = run(args.classified_dir, args.image_dir, args.output_dir, args.font,
                     args.start, args.end, args.workers, args.resume, args.width_pt,
                     args.height_pt, args.dpi, args.merge)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0 if result["status"] == "success" else 2
    except Exception as error:
        print(f"editorial-book-composer: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
