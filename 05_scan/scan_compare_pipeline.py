#!/usr/bin/env python3
"""Run the complete one-page scan-to-digital comparison pipeline."""

from __future__ import annotations

import argparse
import contextlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

import candidate_font_matcher
import coordinate_transform
import line_geometry_matcher
import paddle_line_detector
import paddle_lmstudio_adapter
import scan_word_compare
import word_geometry_detector


def load_catalog(binary: Path, candidate: Path, page_index: int) -> dict[str, Any]:
    process = subprocess.run(
        [str(binary), str(candidate), "--page", str(page_index)],
        check=False,
        capture_output=True,
        text=True,
    )
    if process.returncode != 0:
        raise RuntimeError(process.stderr.strip() or "font catalog failed")
    return json.loads(process.stdout)


def run_pipeline(
    image_path: Path,
    candidate_path: Path,
    catalog_binary: Path,
    base_url: str,
    model: str,
    device: str,
    lang: str,
    page_index: int,
    vlm_provider: Callable[..., list[dict[str, Any]]] = paddle_lmstudio_adapter.run_provider,
    line_provider: Callable[..., list[dict[str, Any]]] = paddle_line_detector.run_provider,
    image_loader: Callable[[Path], Any] | None = None,
    catalog_loader: Callable[[Path, Path, int], dict[str, Any]] = load_catalog,
    word_enricher: Callable[[dict[str, Any], Any], dict[str, Any]] = word_geometry_detector.attach_word_geometry,
) -> dict[str, Any]:
    if image_loader is None:
        import cv2

        image_loader = lambda path: cv2.imread(str(path), cv2.IMREAD_COLOR)
    with contextlib.redirect_stdout(sys.stderr):
        pages = vlm_provider(str(image_path), base_url, model, device)
        detected_pages = line_provider(str(image_path), lang, "PP-OCRv5", device)
    if len(pages) != 1 or len(detected_pages) != 1:
        raise ValueError("esta versão exige exatamente uma página de imagem")
    image = image_loader(image_path)
    if image is None:
        raise ValueError("imagem não pôde ser lida")
    catalog = catalog_loader(catalog_binary, candidate_path, page_index)
    page = line_geometry_matcher.enrich_page(pages[0], detected_pages[0])
    page = word_enricher(page, image)
    page = coordinate_transform.enrich_page(page, catalog)
    page = candidate_font_matcher.enrich_page(page, catalog, str(candidate_path))
    report = scan_word_compare.compare(page, catalog)
    return {"schema_version": 1, "observation": page, "comparison": report}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--catalog-bin", type=Path, default=Path("target/debug/decalque-font-catalog"))
    parser.add_argument("--base-url", default="http://127.0.0.1:1234/v1")
    parser.add_argument("--model", default="paddleocr-vl")
    parser.add_argument("--device", default="cpu")
    parser.add_argument("--lang", default="en")
    parser.add_argument("--page", type=int, default=0)
    args = parser.parse_args()
    try:
        result = run_pipeline(args.image, args.candidate, args.catalog_bin, args.base_url, args.model, args.device, args.lang, args.page)
        json.dump(result, sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"scan-compare-pipeline: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
