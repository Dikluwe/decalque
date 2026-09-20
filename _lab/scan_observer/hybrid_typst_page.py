#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/hybrid-typst-page.md
# @updated 2026-09-16
"""Materialize hybrid scan-line plans as a searchable Typst PDF."""

from __future__ import annotations

import argparse
import json
import math
import re
import subprocess
import sys
from pathlib import Path

from PIL import Image, ImageFont


def typst_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def number(value: float) -> str:
    return f"{value:.5f}".rstrip("0").rstrip(".")


def command(arguments: list[str], label: str) -> str:
    process = subprocess.run(arguments, capture_output=True, text=True, check=False)
    if process.returncode:
        raise RuntimeError(process.stderr.strip() or f"{label} falhou")
    return process.stdout


def resolve(base: Path, value: str) -> Path:
    path = Path(value)
    return path if path.is_absolute() else (base / path).resolve()


def load_manifest(path: Path, font_path: Path) -> tuple[dict, list[dict]]:
    if not font_path.is_file():
        raise ValueError(f"fonte não encontrada: {font_path}")
    payload = json.loads(path.read_text())
    page = payload.get("page", {})
    for field in ("width_pt", "height_pt", "source_width_px", "source_height_px", "dpi"):
        if float(page.get(field, 0)) <= 0:
            raise ValueError(f"page.{field} deve ser positivo")
    lines = payload.get("lines")
    if not isinstance(lines, list) or not lines:
        raise ValueError("manifesto precisa conter linhas")
    base = path.parent
    background_value = payload.get("background") or page.get("image")
    background = resolve(base, background_value or "")
    if not background.is_file():
        raise ValueError("imagem de fundo não encontrada")
    normalized = []
    previous_top = -1.0
    for index, item in enumerate(lines):
        box = item.get("bbox_px")
        if not isinstance(box, list) or len(box) != 4:
            raise ValueError(f"linha {index}: bbox_px inválida")
        box = [float(value) for value in box]
        if box[2] <= box[0] or box[3] <= box[1] or box[1] < previous_top:
            raise ValueError(f"linha {index}: caixa degenerada ou fora de ordem")
        plan_path = resolve(base, item.get("plan", ""))
        if not plan_path.is_file():
            raise ValueError(f"linha {index}: plano não encontrado")
        plan = json.loads(plan_path.read_text())
        if plan.get("status") == "reconstructed" and isinstance(plan.get("text"), str):
            normalized_plan = plan
        elif isinstance(plan.get("transcription"), str) and isinstance(plan.get("segments"), list):
            if "".join(str(segment.get("text", "")) for segment in plan["segments"]) != plan["transcription"]:
                raise ValueError(f"linha {index}: segmentos não reproduzem a transcrição")
            dpi = float(page["dpi"])
            font = plan.get("font", {})
            size_px = float(font.get("size_pt", 0)) * dpi / 72.0
            if size_px <= 0:
                raise ValueError(f"linha {index}: tamanho de fonte inválido")
            spans = []
            for segment in plan["segments"]:
                if segment.get("kind") != "raster":
                    continue
                segment_box = [float(value) for value in segment.get("bbox_px", [])]
                source_box = [float(value) for value in segment.get("source_bbox_px", [])]
                if len(segment_box) != 4 or len(source_box) != 4 or segment_box[2] <= segment_box[0]:
                    raise ValueError(f"linha {index}: segmento raster inválido")
                spans.append({"start": 0, "end": 0, "text": str(segment.get("text", "")),
                              "bounds": {"left_px": segment_box[0] - box[0],
                                         "top_px": segment_box[1] - box[1],
                                         "right_px": segment_box[2] - box[0],
                                         "bottom_px": segment_box[3] - box[1]},
                              "source_bbox_px": source_box})
            normalized_plan = {"status": "reconstructed", "text": plan["transcription"],
                               "baseline_px": float(plan["baseline_px"]) - box[1],
                               "fit": {"font_size_px": size_px,
                                       "tracking_px": float(plan.get("tracking_pt", 0)) * dpi / 72.0,
                                       "horizontal_scale": float(plan.get("scale_x", 1.0)),
                                       "fit_error": 0.0}, "raster_spans": spans}
        else:
            raise ValueError(f"linha {index}: plano incompatível")
        normalized.append({"bbox": box, "plan_path": plan_path, "plan": normalized_plan})
        previous_top = box[1]
    payload["background"] = str(background)
    return payload, normalized


def observed_word_boxes(plan: dict) -> list[tuple[str, float, float]]:
    """Map OCR words to horizontal boxes measured from the source-line ink."""
    matches = list(re.finditer(r"\S+", plan["text"]))
    occurrences = plan.get("occurrences", [])
    if (not matches or not plan.get("source") or
            len(occurrences) != len(plan["text"])):
        return []
    with Image.open(plan["source"]).convert("L") as image:
        occupied = [sum(image.getpixel((x, y)) < 170 for y in range(image.height)) >= 2
                    for x in range(image.width)]
    runs: list[tuple[int, int]] = []
    start = None
    for x, has_ink in enumerate(occupied + [False]):
        if has_ink and start is None:
            start = x
        elif not has_ink and start is not None:
            runs.append((start, x))
            start = None
    if not runs:
        return []
    candidates = [(runs[index + 1][0], runs[index][1], runs[index + 1][0] - runs[index][1])
                  for index in range(len(runs) - 1)
                  if runs[index + 1][0] - runs[index][1] >= 3]
    starts = [runs[0][0]]
    previous = runs[0][0]
    for match in matches[1:]:
        predicted = float(occurrences[match.start()]["bounds"]["left_px"])
        choices = [candidate for candidate in candidates
                   if candidate[0] > previous and abs(candidate[0] - predicted) <= 12]
        if not choices:
            return []
        start, _, _ = min(choices, key=lambda candidate: (abs(candidate[0] - predicted),
                                                           -candidate[2]))
        starts.append(start)
        previous = start
    if len(starts) != len(matches):
        return []
    boxes = []
    for index, (match, left) in enumerate(zip(matches, starts)):
        if index + 1 < len(starts):
            right = max(end for start, end in runs if start < starts[index + 1])
        else:
            right = runs[-1][1]
        boxes.append((match.group(), float(left), float(right)))
    return boxes


def source(lines: list[dict], manifest: dict, font_path: Path) -> str:
    page = manifest["page"]
    sx = float(page["width_pt"]) / float(page["source_width_px"])
    sy = float(page["height_pt"]) / float(page["source_height_px"])
    family = ImageFont.truetype(str(font_path), 20).getname()[0]
    visible_mode = manifest.get("visible_mode")
    background = f'#image({typst_string(manifest["background"])}, width: 100%, height: 100%, fit: "stretch")'
    output = [
        f'#set page(width: {number(float(page["width_pt"]))}pt, height: {number(float(page["height_pt"]))}pt, margin: 0pt)',
        f'#place(top + left)[{background}]',
    ]
    if visible_mode == "vector-debug":
        output.append(
            f'#place(top + left)[#rect(width: {number(float(page["width_pt"]))}pt, '
            f'height: {number(float(page["height_pt"]))}pt, fill: rgb(255, 255, 255, 72%), stroke: none)]')
    for line_index, item in enumerate(lines):
        if visible_mode == "scan-background":
            continue
        x0, y0, x1, y1 = item["bbox"]
        plan = item["plan"]
        fit = plan["fit"]
        size_px = float(fit["font_size_px"])
        tracking_px = float(fit["tracking_px"])
        scale_x = float(fit["horizontal_scale"])
        baseline_local = float(plan["baseline_px"])
        ascent_px, _ = ImageFont.truetype(str(font_path), max(1, int(round(size_px)))).getmetrics()
        text_top_px = y0 + baseline_local - ascent_px
        if visible_mode == "vector-debug":
            box_width = (x1 - x0) * sx
            box_height = (y1 - y0) * sy
            baseline_y = (y0 + baseline_local) * sy
            output.extend([
                f'#place(top + left, dx: {number(x0 * sx)}pt, dy: {number(y0 * sy)}pt)'
                f'[#rect(width: {number(box_width)}pt, height: {number(box_height)}pt, '
                'fill: none, stroke: (paint: blue, thickness: 0.45pt))]',
                f'#place(top + left, dx: {number(x0 * sx)}pt, dy: {number(baseline_y)}pt)'
                f'[#line(length: {number(box_width)}pt, stroke: (paint: green, thickness: 0.35pt))]',
                f'#place(top + left, dx: {number(max(0, x0 * sx - 15))}pt, dy: {number(y0 * sy)}pt)'
                f'[#text(size: 4.5pt, fill: blue, "L{line_index + 3:03d}")]',
            ])
        else:
            output.append(
                f'#place(top + left, dx: {number(x0 * sx)}pt, dy: {number(y0 * sy)}pt)'
                f'[#rect(width: {number((x1-x0)*sx)}pt, height: {number((y1-y0)*sy)}pt, fill: white, stroke: none)]')
        hybrid_artifact = plan.get("artifacts", {}).get("hybrid")
        if hybrid_artifact and visible_mode != "vector-debug":
            hybrid_path = resolve(item["plan_path"].parent, hybrid_artifact)
            if not hybrid_path.is_file():
                raise ValueError(f"aparência híbrida não encontrada: {hybrid_path}")
            output.append(
                f'#place(top + left, dx: {number(x0*sx)}pt, dy: {number(y0*sy)}pt)'
                f'[#image({typst_string(str(hybrid_path))}, width: {number((x1-x0)*sx)}pt, '
                f'height: {number((y1-y0)*sy)}pt, fit: "stretch")]')
            continue
        measured_words = observed_word_boxes(plan) if visible_mode == "vector-debug" else []
        if measured_words:
            pil_font = ImageFont.truetype(str(font_path), max(1, int(round(size_px))))
            for word, local_left, local_right in measured_words:
                natural_width = float(pil_font.getlength(word))
                word_scale = (local_right - local_left) / natural_width if natural_width else 1.0
                output.append(
                    f'#place(top + left, dx: {number((x0 + local_left) * sx)}pt, '
                    f'dy: {number(text_top_px * sy)}pt)'
                    f'[#scale(x: {number(word_scale * 100)}%, origin: top + left)'
                    f'[#box[#text(font: {typst_string(family)}, size: {number(size_px * sy)}pt, '
                    f'fill: red, ligatures: false, {typst_string(word)})]]]')
        else:
            output.append(
                f'#place(top + left, dx: {number(x0 * sx)}pt, dy: {number(text_top_px * sy)}pt)'
                f'[#scale(x: {number(scale_x * 100)}%, origin: top + left)'
                f'[#box[#text(font: {typst_string(family)}, size: {number(size_px * sy)}pt, '
                f'tracking: {number(tracking_px * sx)}pt, '
                f'fill: {"red" if visible_mode == "vector-debug" else "black"}, '
                f'ligatures: false, {typst_string(plan["text"])})]]]')
        for span in plan.get("raster_spans", []):
            bounds = span["bounds"]
            local_x0, local_y0 = float(bounds["left_px"]), float(bounds["top_px"])
            local_x1, local_y1 = float(bounds["right_px"]), float(bounds["bottom_px"])
            raster = resolve(item["plan_path"].parent, span["path"])
            if not raster.is_file():
                raise ValueError(f"recorte raster não encontrado: {raster}")
            output.append(
                f'#place(top + left, dx: {number((x0+local_x0)*sx)}pt, '
                f'dy: {number((y0+local_y0)*sy)}pt)'
                f'[#image({typst_string(str(raster))}, width: {number((local_x1-local_x0)*sx)}pt, '
                f'height: {number((local_y1-local_y0)*sy)}pt, fit: "stretch")]')
    # Canonical transparent layer: PDF text extractors otherwise infer reading
    # order from transformed glyph geometry and may split tightly tracked words.
    canonical_texts = manifest.get("search_lines") or [item["plan"]["text"] for item in lines]
    canonical = "\n".join(
        f'#text(font: "DejaVu Serif", size: 2pt, fill: rgb(0, 0, 0, 0), '
        f'ligatures: false, {typst_string(text)}) #linebreak()'
        for text in canonical_texts)
    output.append(
        f'#place(top + left, dx: 1pt, dy: {number(float(page["height_pt"]) - 120)}pt)'
        f'[#block(width: {number(float(page["width_pt"]) - 2)}pt, height: 119pt, clip: true)'
        f'[\n#set text(size: 1pt)\n#set par(leading: 0pt)\n{canonical}\n]]')
    return "\n\n".join(output) + "\n"


def run(manifest_path: Path, font_path: Path, output_dir: Path,
        visible_mode: str | None = None) -> dict:
    manifest, lines = load_manifest(manifest_path, font_path)
    if visible_mode is not None:
        manifest["visible_mode"] = visible_mode
    output_dir.mkdir(parents=True, exist_ok=True)
    typ_path = output_dir / "page.typ"
    pdf_path = output_dir / "page.pdf"
    raster_prefix = output_dir / "page"
    text_path = output_dir / "extracted.txt"
    legacy_dir = output_dir / "raster"
    from PIL import Image
    with Image.open(manifest["background"]) as background:
        for line_index, item in enumerate(lines):
            for span_index, span in enumerate(item["plan"].get("raster_spans", [])):
                if "source_bbox_px" not in span:
                    continue
                legacy_dir.mkdir(exist_ok=True)
                box = tuple(int(round(value)) for value in span["source_bbox_px"])
                target = legacy_dir / f"line-{line_index:03d}-span-{span_index:03d}.png"
                background.crop(box).save(target)
                span["path"] = str(target.resolve())
    typ_path.write_text(source(lines, manifest, font_path))
    command(["typst", "compile", "--root", "/", "--font-path", str(font_path.parent),
             str(typ_path), str(pdf_path)], "compilação Typst")
    command(["pdftoppm", "-f", "1", "-l", "1", "-r", str(manifest["page"]["dpi"]),
             "-png", "-singlefile", str(pdf_path), str(raster_prefix)], "rasterização PDF")
    extracted = command(["pdftotext", "-layout", str(pdf_path), "-"], "extração textual")
    text_path.write_text(extracted)
    rendered_path = raster_prefix.with_suffix(".png")
    expected = [math.ceil(float(manifest["page"]["width_pt"]) * float(manifest["page"]["dpi"]) / 72.0),
                math.ceil(float(manifest["page"]["height_pt"]) * float(manifest["page"]["dpi"]) / 72.0)]
    with Image.open(rendered_path) as rendered:
        rendered_size = list(rendered.size)
    normalized_extracted = re.sub(r"\s+", " ", extracted).strip()
    searchable = manifest.get("search_lines") or [item["plan"]["text"] for item in lines]
    missing = [text for text in searchable
               if re.sub(r"\s+", " ", text).strip() not in normalized_extracted]
    if missing:
        raise ValueError(f"texto extraído não preservou {len(missing)} linha(s)")
    if rendered_size != expected:
        raise ValueError(f"raster PDF {rendered_size} difere da referência {expected}")
    report = {"schema_version": 1, "status": "success", "pdf": str(pdf_path.resolve()),
              "typst": str(typ_path.resolve()), "rendered": str(rendered_path.resolve()),
              "extracted_text": str(text_path.resolve()), "page_size_px": rendered_size,
              "line_count": len(lines), "searchable_line_count": len(searchable),
              "font": str(font_path.resolve()),
              "visible_text_mode": ("scan-background-with-transparent-search-layer"
                                    if manifest.get("visible_mode") == "scan-background"
                                    else "vector-debug-overlay-with-transparent-search-layer"
                                    if manifest.get("visible_mode") == "vector-debug"
                                    else "validated-hybrid-line-raster-with-transparent-search-layer"),
              "limitations": ["scan-background-for-unmaterialized-zones",
                              "visible-hybrid-lines-are-raster-in-this-first-searchable-pdf"]}
    (output_dir / "report.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    return report


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--font", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--visible-mode", choices=("scan-background", "hybrid", "vector-debug"))
    args = parser.parse_args()
    try:
        report = run(args.manifest, args.font, args.output_dir, args.visible_mode)
        json.dump(report, sys.stdout, ensure_ascii=False, indent=2); sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"hybrid-typst-page: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
