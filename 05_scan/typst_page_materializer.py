#!/usr/bin/env python3
"""Materialize an inspectable scan-page manifest as deterministic Typst source."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def typst_string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def number(value: float) -> str:
    return f"{value:.4f}".rstrip("0").rstrip(".")


def validate_bbox(value: Any) -> list[float]:
    if not isinstance(value, list) or len(value) != 4:
        raise ValueError("bbox_px deve conter quatro números")
    bbox = [float(item) for item in value]
    if bbox[2] <= bbox[0] or bbox[3] <= bbox[1]:
        raise ValueError("bbox_px degenerada")
    return bbox


def load_manifest(path: Path) -> dict[str, Any]:
    manifest = json.loads(path.read_text())
    if manifest.get("schema_version") != 1:
        raise ValueError("schema_version não suportada")
    page = manifest.get("page", {})
    for field in ("width_pt", "height_pt", "source_width_px", "source_height_px"):
        if float(page.get(field, 0)) <= 0:
            raise ValueError(f"page.{field} deve ser positivo")
    if not isinstance(manifest.get("regions"), list):
        raise ValueError("regions deve ser uma lista")
    return manifest


def materialize(manifest: dict[str, Any], manifest_path: Path) -> str:
    page = manifest["page"]
    width_pt, height_pt = float(page["width_pt"]), float(page["height_pt"])
    scale_x = width_pt / float(page["source_width_px"])
    scale_y = height_pt / float(page["source_height_px"])
    lines = [
        f"#set page(width: {number(width_pt)}pt, height: {number(height_pt)}pt, margin: 0pt)",
        "#set text(fill: black)",
    ]
    for region in manifest["regions"]:
        bbox = validate_bbox(region.get("bbox_px"))
        x, y = bbox[0] * scale_x, bbox[1] * scale_y
        width, height = (bbox[2] - bbox[0]) * scale_x, (bbox[3] - bbox[1]) * scale_y
        kind = region.get("kind")
        if kind == "image":
            source = (manifest_path.parent / region["path"]).resolve()
            body = f'#image({typst_string(str(source))}, width: {number(width)}pt, height: {number(height)}pt, fit: "stretch")'
        elif kind == "text":
            font = region.get("font", {})
            family = font.get("family")
            size = float(font.get("size_pt", 0))
            if not family or size <= 0 or "text" not in region:
                raise ValueError("região text exige text, font.family e font.size_pt")
            weight = font.get("weight", "regular")
            style = font.get("style", "normal")
            tracking = float(font.get("tracking_em", 0)) * size
            leading = float(font.get("leading_pt", size * 1.2))
            skew = float(font.get("skew_deg", 0))
            if not -30 <= skew <= 30:
                raise ValueError("font.skew_deg deve estar entre -30 e 30")
            align = region.get("align", "left")
            if align not in {"left", "center", "right"}:
                raise ValueError("align desconhecido")
            text_settings = (
                f'#set text(font: {typst_string(family)}, size: {number(size)}pt, '
                f'weight: {typst_string(weight)}, style: {typst_string(style)}, '
                f'tracking: {number(tracking)}pt, hyphenate: false)\n'
                f'#set par(leading: {number(max(0, leading - size))}pt)\n'
            )
            body = (text_settings + f'#box(width: 10000pt, clip: false)'
                    f'[#text({typst_string(str(region["text"]))})]'
                    if region.get("single_line") else
                    text_settings + f'#align({align})[#text({typst_string(str(region["text"]))})]')
            if skew:
                body = f"#skew(ax: {number(skew)}deg)[\n{body}\n]"
            horizontal_scale = float(region.get("scale_x", 1.0))
            if not 0.25 <= horizontal_scale <= 4.0:
                raise ValueError("scale_x deve estar entre 0.25 e 4.0")
            if horizontal_scale != 1.0:
                body = (f"#scale(x: {number(horizontal_scale * 100)}%, origin: top + left)"
                        f"[\n{body}\n]")
        else:
            raise ValueError(f"kind de região desconhecido: {kind!r}")
        lines.append(
            f"#place(top + left, dx: {number(x)}pt, dy: {number(y)}pt)[\n"
            f"#block(width: {number(width)}pt, height: {number(height)}pt, clip: false)[\n{body}\n]\n]"
        )
    return "\n\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("manifest", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(materialize(load_manifest(args.manifest), args.manifest))
        return 0
    except Exception as error:
        print(f"typst-page-materializer: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
