#!/usr/bin/env python3
"""Resume-safe PaddleOCR physical-line detection for a directory of book pages."""

from __future__ import annotations

import argparse
import contextlib
import json
import sys
import time
from pathlib import Path

from paddle_line_detector import normalize_result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("images", type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--pattern", default="page-*.png")
    parser.add_argument("--start", type=int)
    parser.add_argument("--end", type=int)
    parser.add_argument("--lang", default="en")
    parser.add_argument("--ocr-version", default="PP-OCRv5")
    parser.add_argument("--detector", default="PP-OCRv5_mobile_det")
    parser.add_argument("--recognizer", default="en_PP-OCRv5_mobile_rec")
    parser.add_argument("--recognition-batch-size", type=int, default=16)
    parser.add_argument("--max-height", type=int)
    parser.add_argument("--device", default="cpu")
    args = parser.parse_args()
    pages = sorted(args.images.glob(args.pattern))
    if args.start is not None or args.end is not None:
        selected = []
        for path in pages:
            try:
                number = int(path.stem.rsplit("-", 1)[1])
            except (IndexError, ValueError):
                continue
            if args.start is not None and number < args.start:
                continue
            if args.end is not None and number > args.end:
                continue
            selected.append(path)
        pages = selected
    if not pages:
        print("paddle-book-line-detector: nenhuma página encontrada", file=sys.stderr)
        return 2
    args.output_dir.mkdir(parents=True, exist_ok=True)
    pending = [path for path in pages if not (args.output_dir / f"{path.stem}.json").is_file()]
    print(json.dumps({"status": "starting", "pages": len(pages), "pending": len(pending)}),
          flush=True)
    if not pending:
        return 0
    from paddleocr import PaddleOCR
    with contextlib.redirect_stdout(sys.stderr):
        pipeline = PaddleOCR(
            lang=args.lang, ocr_version=args.ocr_version, device=args.device,
            text_detection_model_name=args.detector,
            text_recognition_model_name=args.recognizer,
            textline_orientation_batch_size=args.recognition_batch_size,
            text_recognition_batch_size=args.recognition_batch_size,
            enable_mkldnn=False, use_doc_orientation_classify=False,
            use_doc_unwarping=False, use_textline_orientation=False,
            return_word_box=False,
        )
    started = time.monotonic()
    for completed, image_path in enumerate(pending, 1):
        page_started = time.monotonic()
        provider_input: object = str(image_path)
        scale_x = scale_y = 1.0
        if args.max_height:
            import numpy as np
            from PIL import Image
            with Image.open(image_path) as source:
                if source.height > args.max_height:
                    target_width = round(source.width * args.max_height / source.height)
                    resized = source.convert("RGB").resize((target_width, args.max_height),
                                                            Image.Resampling.LANCZOS)
                    provider_input = np.asarray(resized)
                    scale_x = source.width / target_width
                    scale_y = source.height / args.max_height
        with contextlib.redirect_stdout(sys.stderr):
            results = pipeline.predict(provider_input, return_word_box=False)
            normalized = [normalize_result(result.json) for result in results]
        if scale_x != 1.0 or scale_y != 1.0:
            for page in normalized:
                for line in page["lines"]:
                    x0, y0, x1, y1 = line["bbox"]
                    line["bbox"] = [round(x0 * scale_x), round(y0 * scale_y),
                                    round(x1 * scale_x), round(y1 * scale_y)]
                    line["polygon"] = [[round(x * scale_x), round(y * scale_y)]
                                       for x, y in line["polygon"]]
        payload = {"schema_version": 1, "provider": "paddleocr",
                   "source": str(image_path.resolve()), "pages": normalized}
        target = args.output_dir / f"{image_path.stem}.json"
        target.write_text(json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n",
                          encoding="utf-8")
        line_count = sum(len(page["lines"]) for page in normalized)
        print(json.dumps({"status": "page-complete", "page": image_path.stem,
                          "completed": completed, "pending_total": len(pending),
                          "lines": line_count,
                          "page_seconds": round(time.monotonic() - page_started, 2),
                          "elapsed_seconds": round(time.monotonic() - started, 2)}), flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
