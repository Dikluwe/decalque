#!/usr/bin/env python3
# Crystalline Lineage
# @prompt 00_nucleo/prompts/chapter-book-segmentation.md
# @layer L5
"""Split materialized page PDFs into validated editorial chapter artifacts."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


def validate_manifest(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    if manifest.get("schema_version") != 1:
        raise ValueError("schema_version não suportada")
    total = int(manifest.get("total_pages", 0))
    chapters = manifest.get("chapters")
    if total <= 0 or not isinstance(chapters, list) or not chapters:
        raise ValueError("total_pages e chapters devem ser válidos")
    expected = 1
    ids: set[str] = set()
    for chapter in chapters:
        identifier = chapter.get("id")
        if not identifier or identifier in ids:
            raise ValueError("ids de capítulo devem ser únicos")
        ids.add(identifier)
        if not chapter.get("title") or not chapter.get("evidence"):
            raise ValueError("title e evidence são obrigatórios")
        start, end = int(chapter.get("start", 0)), int(chapter.get("end", 0))
        if start != expected or end < start:
            raise ValueError("intervalos devem ser contíguos, ordenados e não sobrepostos")
        expected = end + 1
    if expected != total + 1:
        raise ValueError("intervalos não cobrem total_pages exatamente")
    return chapters


def chapter_report(chapter: dict[str, Any], review_pages: set[int]) -> dict[str, Any]:
    start, end = int(chapter["start"]), int(chapter["end"])
    return {"id": chapter["id"], "title": chapter["title"], "start_page": start,
            "end_page": end, "page_count": end - start + 1,
            "evidence": chapter["evidence"],
            "review_pages": sorted(review_pages & set(range(start, end + 1)))}


def _merge(inputs: list[Path], target: Path) -> float:
    missing = [path for path in inputs if not path.is_file()]
    if missing:
        raise ValueError(f"PDFs de página ausentes: {', '.join(str(p) for p in missing[:3])}")
    started = time.perf_counter()
    process = subprocess.run(["pdfunite", *map(str, inputs), str(target)],
                             capture_output=True, text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or "pdfunite falhou")
    return time.perf_counter() - started


def split(manifest_path: Path, pages_dir: Path, book_report_path: Path,
          output_dir: Path) -> dict[str, Any]:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    chapters = validate_manifest(manifest)
    book_report = json.loads(book_report_path.read_text(encoding="utf-8"))
    review_pages = set(int(value) for value in book_report.get("text_review_pages", []))
    output_dir.mkdir(parents=True, exist_ok=True)
    results = []
    for chapter in chapters:
        report = chapter_report(chapter, review_pages)
        target = output_dir / f'{chapter["id"]}.pdf'
        inputs = [pages_dir / f"page-{number:03d}" / "page.pdf"
                  for number in range(report["start_page"], report["end_page"] + 1)]
        report["merge_seconds"] = round(_merge(inputs, target), 6)
        report["pdf"] = str(target.resolve())
        report["status"] = "review" if report["review_pages"] else "ready"
        results.append(report)
    summary = {"schema_version": 1, "book_id": manifest.get("book_id"),
               "status": "success", "chapter_count": len(results),
               "total_pages": manifest["total_pages"],
               "review_chapter_count": sum(item["status"] == "review" for item in results),
               "chapters": results}
    (output_dir / "chapters-report.json").write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n")
    lines = ["# Capítulos para revisão", "",
             "| Arquivo | Páginas físicas | Estado | Páginas para revisão |",
             "|---|---:|---|---|"]
    for item in results:
        review = ", ".join(map(str, item["review_pages"])) or "-"
        lines.append(f'| `{item["id"]}.pdf` — {item["title"]} | '
                     f'{item["start_page"]}–{item["end_page"]} | {item["status"]} | {review} |')
    (output_dir / "README.md").write_text("\n".join(lines) + "\n")
    return summary


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("pages_dir", type=Path)
    parser.add_argument("book_report", type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    try:
        result = split(args.manifest, args.pages_dir, args.book_report, args.output_dir)
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    except Exception as error:
        print(f"chapter-book-splitter: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
