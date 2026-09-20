#!/usr/bin/env python3
"""Enrich scan tokens with structural font evidence from a candidate PDF catalog."""

from __future__ import annotations

import argparse
import copy
import difflib
import json
import re
import sys
import unicodedata
from pathlib import Path
from typing import Any


SUBSET_PREFIX = re.compile(r"^[A-Z]{6}\+")
STYLE_SUFFIX = re.compile(
    r"[-,](?:regular|roman|bold|semibold|demibold|light|italic|oblique|bolditalic|boldoblique)$",
    re.IGNORECASE,
)
ENCODING_SUFFIX = re.compile(r"-(?:Identity-[HV]|WinAnsiEncoding)$", re.IGNORECASE)


def font_signature(glyph: dict[str, Any]) -> tuple[Any, ...]:
    return glyph.get("font_ref"), glyph.get("base_font"), glyph.get("font_size_pt")


def font_hypothesis(glyph: dict[str, Any], candidate: str) -> dict[str, Any]:
    declared = glyph["base_font"]
    name = ENCODING_SUFFIX.sub("", SUBSET_PREFIX.sub("", declared))
    lower = name.lower()
    family = STYLE_SUFFIX.sub("", name)
    style = "italic" if "italic" in lower or "oblique" in lower else "normal"
    if "semibold" in lower or "demibold" in lower:
        weight = "semibold"
    elif "bold" in lower:
        weight = "bold"
    elif "light" in lower:
        weight = "light"
    else:
        weight = "regular"
    return {
        "status": "inferred",
        "family": family,
        "style": style,
        "weight": weight,
        "size_px": None,
        "size_pt": glyph["font_size_pt"],
        "confidence": 1.0,
        "evidence": [
            {
                "method": "candidate-text-alignment",
                "candidate": candidate,
                "font_ref": glyph["font_ref"],
                "declared_base_font": declared,
            }
        ],
    }


def logical_glyph_order(glyphs: list[dict[str, Any]]) -> list[int]:
    order: list[int] = []
    start = 0
    while start < len(glyphs):
        first = glyphs[start]
        first_y = (first.get("position") or [None, None])[1]
        end = start + 1
        while end < len(glyphs):
            current = glyphs[end]
            current_y = (current.get("position") or [None, None])[1]
            if current.get("font_ref") != first.get("font_ref") or current_y != first_y:
                break
            end += 1
        classes = {
            unicodedata.bidirectional(character)
            for glyph in glyphs[start:end]
            for character in (glyph.get("text") or "")
            if unicodedata.bidirectional(character) in {"L", "R", "AL"}
        }
        indexes = list(range(start, end))
        if classes and classes <= {"R", "AL"}:
            indexes.reverse()
        order.extend(indexes)
        start = end
    return order


def compact_candidate(glyphs: list[dict[str, Any]]) -> tuple[str, list[int]]:
    characters: list[str] = []
    owners: list[int] = []
    for index in logical_glyph_order(glyphs):
        glyph = glyphs[index]
        for character in glyph.get("text") or "":
            if not character.isspace():
                characters.append(character)
                owners.append(index)
    return "".join(characters), owners


def compact_scan(page: dict[str, Any]) -> tuple[str, list[tuple[dict[str, Any], int]]]:
    characters: list[str] = []
    owners: list[tuple[dict[str, Any], int]] = []
    for region in page.get("regions", []):
        for line in region.get("lines", []):
            for token in line.get("tokens", []):
                for offset, character in enumerate(token.get("text") or ""):
                    if not character.isspace():
                        characters.append(character)
                        owners.append((token, offset))
    return "".join(characters), owners


def ambiguous_token(
    token_text: str,
    candidate_text: str,
    candidate_owners: list[int],
    glyphs: list[dict[str, Any]],
) -> bool:
    needle = "".join(character for character in token_text if not character.isspace())
    if not needle:
        return False
    signatures = set()
    start = 0
    while (found := candidate_text.find(needle, start)) >= 0:
        indexes = {candidate_owners[index] for index in range(found, found + len(needle))}
        occurrence = {font_signature(glyphs[index]) for index in indexes}
        if len(occurrence) == 1:
            signatures.update(occurrence)
        start = found + 1
    return len(signatures) > 1


def enrich_page(
    page: dict[str, Any], catalog: dict[str, Any], candidate: str
) -> dict[str, Any]:
    output = copy.deepcopy(page)
    glyphs = catalog.get("glyphs", [])
    scan_text, scan_owners = compact_scan(output)
    candidate_text, candidate_owners = compact_candidate(glyphs)
    character_map: dict[int, int] = {}
    matcher = difflib.SequenceMatcher(None, scan_text, candidate_text, autojunk=False)
    for scan_start, candidate_start, size in matcher.get_matching_blocks():
        for delta in range(size):
            character_map[scan_start + delta] = candidate_start + delta

    indexes_by_token: dict[int, tuple[dict[str, Any], list[int]]] = {}
    for scan_index, (token, _) in enumerate(scan_owners):
        entry = indexes_by_token.setdefault(id(token), (token, []))
        entry[1].append(scan_index)

    for token, scan_indexes in indexes_by_token.values():
        if token.get("kind") == "whitespace" or not scan_indexes:
            continue
        if any(index not in character_map for index in scan_indexes):
            continue
        candidate_indexes = [character_map[index] for index in scan_indexes]
        if candidate_indexes != list(
            range(candidate_indexes[0], candidate_indexes[0] + len(candidate_indexes))
        ):
            continue
        compact_token = "".join(
            character for character in token.get("text", "") if not character.isspace()
        )
        if candidate_text[candidate_indexes[0] : candidate_indexes[-1] + 1] != compact_token:
            continue
        glyph_indexes = {
            candidate_owners[index] for index in candidate_indexes
        }
        signatures = {font_signature(glyphs[index]) for index in glyph_indexes}
        if len(signatures) != 1:
            continue
        glyph = glyphs[min(glyph_indexes)]
        if not glyph.get("base_font") or glyph.get("font_size_pt") is None:
            continue
        if ambiguous_token(token.get("text", ""), candidate_text, candidate_owners, glyphs):
            continue
        token["font"] = font_hypothesis(glyph, candidate)
    return output


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("scan_json", type=Path)
    parser.add_argument("candidate_catalog", type=Path)
    parser.add_argument("--candidate-label")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        pages = json.loads(args.scan_json.read_text(encoding="utf-8"))
        catalog = json.loads(args.candidate_catalog.read_text(encoding="utf-8"))
        label = args.candidate_label or str(args.candidate_catalog)
        output = [enrich_page(page, catalog, label) for page in pages]
        json.dump(output, sys.stdout, ensure_ascii=False, separators=(",", ":"))
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"candidate-font-matcher: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
