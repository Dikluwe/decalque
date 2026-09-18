#!/usr/bin/env python3
"""Rank Google Fonts candidates against a single-line scan sample."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import sys
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image, ImageDraw, ImageFont


OFFICIAL_API = "https://www.googleapis.com/webfonts/v1/webfonts"
MASK_SIZE = 256
SECRET_FILE = Path.home() / ".config/decalque/secrets.env"


def protected_api_key(path: Path = SECRET_FILE) -> str | None:
    """Read the API key as data, never by executing the secrets file."""
    if not path.exists():
        return None
    metadata = path.stat()
    if not stat.S_ISREG(metadata.st_mode):
        raise ValueError(f"arquivo de chave não é regular: {path}")
    if hasattr(os, "getuid") and metadata.st_uid != os.getuid():
        raise ValueError(f"arquivo de chave pertence a outro usuário: {path}")
    if metadata.st_mode & (stat.S_IRWXG | stat.S_IRWXO):
        raise ValueError(f"permissões inseguras no arquivo de chave; use chmod 600 {path}")
    assignment = re.compile(
        r"^(?:export\s+)?GOOGLE_FONTS_API_KEY=(?:['\"])?([A-Za-z0-9_-]+)(?:['\"])?\s*$"
    )
    for line in path.read_text(encoding="utf-8").splitlines():
        match = assignment.fullmatch(line.strip())
        if match:
            return match.group(1)
    raise ValueError(f"GOOGLE_FONTS_API_KEY não encontrada em {path}")


def default_api_key() -> str | None:
    return os.environ.get("GOOGLE_FONTS_API_KEY") or protected_api_key()


def normalized_ink(image: Image.Image) -> np.ndarray:
    gray = image.convert("L")
    array = np.asarray(gray, dtype=np.float64)
    ink = 1.0 - array / 255.0
    active = np.argwhere(ink > 0.08)
    if not len(active):
        return np.zeros((MASK_SIZE, MASK_SIZE), dtype=np.float64)
    y0, x0 = active.min(axis=0)
    y1, x1 = active.max(axis=0) + 1
    crop = gray.crop((int(x0), int(y0), int(x1), int(y1)))
    scale = min(MASK_SIZE / crop.width, MASK_SIZE / crop.height)
    resized = crop.resize((max(1, round(crop.width * scale)), max(1, round(crop.height * scale))),
                          Image.Resampling.LANCZOS)
    canvas = Image.new("L", (MASK_SIZE, MASK_SIZE), 255)
    canvas.paste(resized, ((MASK_SIZE - resized.width) // 2, (MASK_SIZE - resized.height) // 2))
    return 1.0 - np.asarray(canvas, dtype=np.float64) / 255.0


def similarity(left: np.ndarray, right: np.ndarray) -> float:
    union = np.maximum(left, right).sum()
    return 0.0 if union == 0 else float(np.minimum(left, right).sum() / union)


def render_text(text: str, font_path: Path) -> Image.Image:
    font = ImageFont.truetype(str(font_path), 180)
    probe = Image.new("L", (1, 1), 255)
    bounds = ImageDraw.Draw(probe).textbbox((0, 0), text, font=font)
    width, height = bounds[2] - bounds[0], bounds[3] - bounds[1]
    image = Image.new("L", (width + 40, height + 40), 255)
    ImageDraw.Draw(image).text((20 - bounds[0], 20 - bounds[1]), text, font=font, fill=0)
    return image


def catalog_url(api_url: str, key: str, category: str, sort: str) -> str:
    separator = "&" if "?" in api_url else "?"
    return api_url + separator + urllib.parse.urlencode({"key": key, "category": category, "sort": sort})


def fetch_json(url: str) -> dict[str, Any]:
    try:
        with urllib.request.urlopen(url, timeout=60) as response:
            return json.load(response)
    except Exception as error:
        raise RuntimeError("falha ao consultar catálogo Google Fonts") from error


def validate_api_url(url: str, allow_insecure_localhost: bool) -> None:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme == "https":
        return
    if allow_insecure_localhost and parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost"}:
        return
    raise ValueError("URL da API deve usar HTTPS")


def safe_font_url(url: str, allow_insecure_localhost: bool) -> str:
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme == "http" and parsed.hostname == "fonts.gstatic.com":
        return urllib.parse.urlunparse(parsed._replace(scheme="https"))
    if parsed.scheme == "https":
        return url
    if allow_insecure_localhost and parsed.scheme == "http" and parsed.hostname in {"127.0.0.1", "localhost"}:
        return url
    raise ValueError("URL de fonte insegura rejeitada")


def download_font(url: str, cache_dir: Path, allow_insecure_localhost: bool,
                  timeout: float = 10.0) -> Path:
    safe_url = safe_font_url(url, allow_insecure_localhost)
    suffix = Path(urllib.parse.urlparse(safe_url).path).suffix or ".font"
    target = cache_dir / f"{hashlib.sha256(safe_url.encode()).hexdigest()}{suffix}"
    if not target.exists():
        cache_dir.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(safe_url, timeout=timeout) as response:
            target.write_bytes(response.read())
    return target


def discover(sample_path: Path, text: str, api_url: str, api_key: str | None, category: str,
             sort: str, variant: str, limit: int, cache_dir: Path,
             allow_insecure_localhost: bool = False, download_timeout: float = 10.0,
             progress: bool = False) -> dict[str, Any]:
    if not api_key:
        raise ValueError("GOOGLE_FONTS_API_KEY não configurada")
    if limit < 1:
        raise ValueError("limit deve ser positivo")
    validate_api_url(api_url, allow_insecure_localhost)
    catalog = fetch_json(catalog_url(api_url, api_key, category, sort))
    with Image.open(sample_path) as sample:
        target_ink = normalized_ink(sample)
    ranked, rejected = [], []
    selected = catalog.get("items", [])[:limit]
    for index, item in enumerate(selected, 1):
        family = item.get("family", "")
        if progress:
            print(f"[{index}/{len(selected)}] {family}", file=sys.stderr, flush=True)
        files = item.get("files", {})
        if variant not in files:
            rejected.append({"family": family, "reason": "variant_missing"})
            continue
        try:
            source_url = safe_font_url(files[variant], allow_insecure_localhost)
            font_path = download_font(source_url, cache_dir, allow_insecure_localhost,
                                      download_timeout)
            score = similarity(target_ink, normalized_ink(render_text(text, font_path)))
            ranked.append({"family": family, "variant": variant, "category": item.get("category"),
                           "similarity": round(score, 6), "source_url": source_url,
                           "font_path": str(font_path.resolve())})
        except Exception as error:
            rejected.append({"family": family, "reason": "font_unusable", "detail": str(error)})
    ranked.sort(key=lambda item: (-item["similarity"], item["family"]))
    return {"schema_version": 1, "status": "ranked" if ranked else "unknown",
            "query": {"category": category, "sort": sort, "variant": variant, "limit": limit,
                      "text": text}, "ranked": ranked, "rejected": rejected}


def load_evidence(path: Path) -> list[dict[str, Any]]:
    document = json.loads(path.read_text())
    samples = document.get("samples")
    if not isinstance(samples, list) or not samples:
        raise ValueError("evidence.samples deve ser uma lista não vazia")
    allowed_dimensions = {"family", "weight", "style"}
    normalized = []
    for index, sample in enumerate(samples):
        dimensions = sample.get("dimensions", [])
        variants = sample.get("variants", [])
        weight = float(sample.get("weight", 0))
        if (not sample.get("path") or not sample.get("text") or weight <= 0 or
                not dimensions or not set(dimensions) <= allowed_dimensions or not variants):
            raise ValueError(f"evidência {index} inválida")
        normalized.append({"id": sample.get("id", f"sample-{index + 1:03d}"),
                           "path": (path.parent / sample["path"]).resolve(),
                           "text": str(sample["text"]), "weight": weight,
                           "dimensions": list(dict.fromkeys(dimensions)),
                           "variants": list(dict.fromkeys(variants)),
                           "required": bool(sample.get("required", True))})
    return normalized


def weighted_score(evidence: list[dict[str, Any]], dimension: str | None = None) -> float | None:
    selected = [item for item in evidence if dimension is None or dimension in item["dimensions"]]
    total = sum(item["weight"] for item in selected)
    if not selected or total == 0:
        return None
    return round(sum(item["similarity"] * item["weight"] for item in selected) / total, 6)


def discover_evidence(evidence_path: Path, api_url: str, api_key: str | None, category: str,
                      sort: str, limit: int, cache_dir: Path,
                      allow_insecure_localhost: bool = False, download_timeout: float = 10.0,
                      progress: bool = False) -> dict[str, Any]:
    if not api_key:
        raise ValueError("GOOGLE_FONTS_API_KEY não configurada")
    if limit < 1:
        raise ValueError("limit deve ser positivo")
    validate_api_url(api_url, allow_insecure_localhost)
    samples = load_evidence(evidence_path)
    target_inks = {}
    for sample in samples:
        with Image.open(sample["path"]) as image:
            target_inks[sample["id"]] = normalized_ink(image)
    catalog = fetch_json(catalog_url(api_url, api_key, category, sort))
    ranked, rejected = [], []
    selected = catalog.get("items", [])[:limit]
    for index, item in enumerate(selected, 1):
        family = item.get("family", "")
        if progress:
            print(f"[{index}/{len(selected)}] {family}", file=sys.stderr, flush=True)
        files = item.get("files", {})
        scores, missing = [], []
        for sample in samples:
            variants = []
            for variant in sample["variants"]:
                if variant not in files:
                    continue
                try:
                    source_url = safe_font_url(files[variant], allow_insecure_localhost)
                    font_path = download_font(source_url, cache_dir, allow_insecure_localhost,
                                              download_timeout)
                    score = similarity(target_inks[sample["id"]],
                                       normalized_ink(render_text(sample["text"], font_path)))
                    variants.append((score, variant, source_url, font_path))
                except Exception:
                    continue
            if not variants:
                missing.append(sample["id"])
                continue
            score, variant, source_url, font_path = max(variants, key=lambda value: (value[0], value[1]))
            scores.append({"id": sample["id"], "text": sample["text"], "weight": sample["weight"],
                           "dimensions": sample["dimensions"], "variant": variant,
                           "similarity": round(score, 6), "source_url": source_url,
                           "font_path": str(font_path.resolve())})
        required_missing = [sample["id"] for sample in samples if sample["required"] and sample["id"] in missing]
        if required_missing:
            rejected.append({"family": family, "reason": "required_evidence_missing",
                             "samples": required_missing})
            continue
        ranked.append({"family": family, "category": item.get("category"),
                       "overall_score": weighted_score(scores),
                       "family_score": weighted_score(scores, "family"),
                       "weight_score": weighted_score(scores, "weight"),
                       "style_score": weighted_score(scores, "style"),
                       "evidence": scores, "optional_missing": missing})
    ranked.sort(key=lambda item: (-item["overall_score"], item["family"]))
    return {"schema_version": 2, "status": "ranked" if ranked else "unknown",
            "query": {"category": category, "sort": sort, "limit": limit,
                      "evidence": str(evidence_path.resolve())},
            "ranked": ranked, "rejected": rejected}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("sample", type=Path, nargs="?")
    parser.add_argument("--text")
    parser.add_argument("--evidence", type=Path)
    parser.add_argument("--api-url", default=OFFICIAL_API)
    parser.add_argument("--api-key", default=None)
    parser.add_argument("--category", default="serif")
    parser.add_argument("--sort", default="popularity")
    parser.add_argument("--variant", default="700")
    parser.add_argument("--limit", type=int, default=30)
    parser.add_argument("--cache-dir", type=Path, default=Path("output/google-fonts-cache"))
    parser.add_argument("--download-timeout", type=float, default=10.0,
                        help="limite em segundos para cada download de fonte (padrão: 10)")
    parser.add_argument("--progress", action="store_true",
                        help="mostra no stderr a família que está sendo processada")
    parser.add_argument("--allow-insecure-localhost", action="store_true")
    args = parser.parse_args()
    try:
        api_key = args.api_key or default_api_key()
        if args.download_timeout <= 0:
            raise ValueError("download-timeout deve ser positivo")
        if args.evidence:
            result = discover_evidence(args.evidence, args.api_url, api_key, args.category,
                                       args.sort, args.limit, args.cache_dir,
                                       args.allow_insecure_localhost, args.download_timeout,
                                       args.progress)
        elif args.sample and args.text:
            result = discover(args.sample, args.text, args.api_url, api_key, args.category,
                              args.sort, args.variant, args.limit, args.cache_dir,
                              args.allow_insecure_localhost, args.download_timeout, args.progress)
        else:
            raise ValueError("informe sample + --text ou --evidence")
        json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    except Exception as error:
        print(f"google-fonts-discovery: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
