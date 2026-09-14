#!/usr/bin/env python3
"""Rasterize and compare every scanned PDF page sequentially."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


PAGES = re.compile(r"^Pages:\s+(\d+)\s*$", re.MULTILINE)


def page_count(pdf: Path) -> int:
    process = subprocess.run(
        ["pdfinfo", str(pdf)], capture_output=True, text=True, check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(process.stderr.strip() or "pdfinfo failed")
    match = PAGES.search(process.stdout)
    if match is None or int(match.group(1)) < 1:
        raise RuntimeError("pdfinfo did not report a positive page count")
    return int(match.group(1))


def render_page(pdf: Path, page_number: int, dpi: int, output: Path) -> None:
    process = subprocess.run(
        [
            "pdftoppm", "-f", str(page_number), "-l", str(page_number),
            "-singlefile", "-png", "-r", str(dpi), str(pdf), str(output.with_suffix("")),
        ],
        capture_output=True, text=True, check=False,
    )
    if process.returncode != 0 or not output.is_file():
        raise RuntimeError(process.stderr.strip() or "pdftoppm did not create the page image")


def compare_page(
    python: Path, pipeline: Path, image: Path, candidate: Path, catalog_binary: Path,
    base_url: str, model: str, device: str, lang: str, page_index: int,
) -> dict[str, Any]:
    process = subprocess.run(
        [
            str(python), str(pipeline), str(image), str(candidate),
            "--catalog-bin", str(catalog_binary), "--base-url", base_url,
            "--model", model, "--device", device, "--lang", lang,
            "--page", str(page_index),
        ],
        capture_output=True, text=True, check=False,
    )
    if process.stderr:
        print(process.stderr, file=sys.stderr, end="")
    if process.returncode != 0:
        raise RuntimeError(f"page pipeline exited with code {process.returncode}")
    try:
        return json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"page pipeline emitted invalid JSON: {error}") from error


def aggregate(statuses: list[str]) -> str:
    if "violated" in statuses:
        return "violated"
    if not statuses or "unknown" in statuses:
        return "unknown"
    return "preserved"


def run(args: argparse.Namespace) -> dict[str, Any]:
    total = page_count(args.scan_pdf)
    pages = []
    with tempfile.TemporaryDirectory(prefix="decalque-document-") as directory_name:
        directory = Path(directory_name)
        for page_index in range(total):
            page_number = page_index + 1
            print(f"scan-compare-document: page {page_number}/{total}", file=sys.stderr)
            image = directory / f"page-{page_number:06d}.png"
            try:
                render_page(args.scan_pdf, page_number, args.dpi, image)
                result = compare_page(
                    args.python, args.pipeline, image, args.candidate_pdf,
                    args.catalog_bin, args.base_url, args.model, args.device,
                    args.lang, page_index,
                )
            except Exception as error:
                raise RuntimeError(f"page {page_number}/{total}: {error}") from error
            pages.append({"page_index": page_index, "result": result})

    statuses = [
        page["result"].get("comparison", {}).get("verdict", {}).get("status", "unknown")
        for page in pages
    ]
    return {
        "schema_version": 1,
        "page_count": total,
        "dpi": args.dpi,
        "verdict": {"status": aggregate(statuses), "page_statuses": statuses},
        "pages": pages,
    }


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser()
    result.add_argument("scan_pdf", type=Path)
    result.add_argument("candidate_pdf", type=Path)
    result.add_argument("--dpi", type=int, default=200)
    result.add_argument("--python", type=Path, default=Path(sys.executable))
    result.add_argument("--pipeline", type=Path, default=Path(__file__).parent / "scan_compare_pipeline.py")
    result.add_argument("--catalog-bin", type=Path, default=Path("target/debug/decalque-font-catalog"))
    result.add_argument("--base-url", default="http://127.0.0.1:1234/v1")
    result.add_argument("--model", default="paddleocr-vl")
    result.add_argument("--device", default="cpu")
    result.add_argument("--lang", default="en")
    return result


def main() -> int:
    args = parser().parse_args()
    if args.dpi < 1:
        print("scan-compare-document: dpi must be positive", file=sys.stderr)
        return 2
    try:
        json.dump(run(args), sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"scan-compare-document: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
