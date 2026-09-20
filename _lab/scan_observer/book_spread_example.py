#!/usr/bin/env python3
"""Materialize OCR lines with mirrored book masters for a visual spread experiment."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path


def string(value: str) -> str:
    return json.dumps(value, ensure_ascii=False)


def num(value: float) -> str:
    return f"{value:.4f}".rstrip("0").rstrip(".")


def mirrored(master: dict, width: float) -> dict:
    result = dict(master)
    center = master.get("pagination_center_pt", [None, None])
    result["pagination_center_pt"] = [width - center[0], center[1]] if center[0] else [None, None]
    return result


def render_page(page: dict, master: dict, side: str, width: float, height: float,
                family: str, size: float, title_size: float,
                title_family: str | None = None, title_weight: str = "bold") -> str:
    lines = [line.strip() for line in page["text"].splitlines() if line.strip()]
    printed = lines.pop() if lines and lines[-1].isdigit() else str(page.get("printed_page", ""))
    title = lines.pop(0) if lines else ""
    extras = []
    if page.get("page_number") == 13:
        extras = lines[:2]
        lines = lines[2:]
    outer, inner = master["outer_margin_pt"], master["inner_margin_pt"]
    left = outer if side == "left" else inner
    top = master["top_margin_pt"]
    leading = master["leading_pt"]
    body_width = master["body_width_pt"]
    parts = [f'#set page(width: {num(width)}pt, height: {num(height)}pt, margin: 0pt)',
             '#set text(fill: black)',
             f'#place(top + left, dx: {num(left)}pt, dy: {num(top - leading)}pt)'
             f'[#text(font: {string(title_family or family)}, size: {num(title_size)}pt, '
             f'weight: {string(title_weight)}, {string(title)})]']
    for index, line in enumerate(extras):
        parts.append(f'#place(top + left, dx: {num(left)}pt, dy: {num(top + index * leading)}pt)'
                     f'[#text(font: {string(family)}, size: {num(size)}pt, '
                     f'style: {string("italic" if index == 0 else "normal")}, {string(line)})]')
    paragraph_prefixes = ("In 1969,", "About 15 years", "Its first edition", "Today, TRIZ",
                          "Simon Litvin is", "Altshuller responded", "He considered himself",
                          "Altshuller held", "Thanks to TRIZ")
    paragraphs: list[str] = []
    for line in lines:
        if not paragraphs or line.startswith(paragraph_prefixes):
            paragraphs.append(line)
        else:
            paragraphs[-1] += " " + line
    body_y = top + len(extras) * leading
    content = "\n".join(f'#par[#text({string(paragraph)})]' for paragraph in paragraphs)
    parts.append(f'#place(top + left, dx: {num(left)}pt, dy: {num(body_y)}pt)'
                 f'[#block(width: {num(body_width)}pt, height: {num(height - body_y - 30)}pt, clip: false)['
                 f'#set text(font: {string(family)}, size: {num(size)}pt)\n'
                 f'#set par(justify: true, first-line-indent: {num(master["first_line_indent_pt"])}pt, '
                 f'leading: {num(max(0, leading - size))}pt, '
                 f'spacing: {num(float(master.get("paragraph_spacing_pt", 0)))}pt)\n{content}]]')
    footer = master.get("pagination_center_pt", [None, None])
    if printed and footer[0] is not None:
        parts.append(f'#place(top + left, dx: {num(footer[0] - size / 2)}pt, dy: {num(footer[1] - size / 2)}pt)'
                     f'[#text(font: {string(family)}, size: {num(size)}pt, weight: "bold", {string(printed)})]')
    return "\n".join(parts)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ocr", type=Path)
    parser.add_argument("masters", type=Path)
    parser.add_argument("--pages", type=int, nargs=2, required=True)
    parser.add_argument("--font", required=True)
    parser.add_argument("--size", type=float, default=11.5)
    parser.add_argument("--title-size", type=float, default=12.8)
    args = parser.parse_args()
    try:
        ocr = json.loads(args.ocr.read_text())
        profile = json.loads(args.masters.read_text())
        width, height = profile["page"]["width_pt"], profile["page"]["height_pt"]
        left_master = profile["masters"]["left"]
        right_master = profile["masters"]["right"]
        if not right_master.get("sample_count"):
            right_master = mirrored(left_master, width)
        by_number = {page["page_number"]: page for page in ocr["pages"]}
        rendered = []
        for number in args.pages:
            page = by_number[number]
            printed = int(page["text"].splitlines()[-1])
            side = "left" if printed % 2 == 0 else "right"
            master = left_master if side == "left" else right_master
            rendered.append(render_page(page, master, side, width, height,
                                        args.font, args.size, args.title_size))
        sys.stdout.write("\n#pagebreak()\n".join(rendered) + "\n")
        return 0
    except Exception as error:
        print(f"book-spread-example: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
