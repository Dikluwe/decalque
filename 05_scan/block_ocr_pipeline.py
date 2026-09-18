#!/usr/bin/env python3
# Crystalline Lineage
# @prompt 00_nucleo/prompts/block-ocr-pipeline.md
# @layer L5
"""Execute declarative OCR experiments as typed artifact blocks."""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import re
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

import numpy as np
from PIL import Image
from scipy.ndimage import label


IMAGE_TAG = re.compile(
    r'<img\s+src=["\']images/bbox_(\d+)_(\d+)_(\d+)_(\d+)\.[^"\']+["\']\s*/?>',
    re.IGNORECASE,
)


class PipelineError(ValueError):
    pass


@dataclass(frozen=True)
class BlockType:
    inputs: frozenset[str]
    outputs: frozenset[str]
    execute: Callable[[dict[str, Any], dict[str, Any], Path], dict[str, Any]]


def _load_module(name: str, filename: str):
    path = Path(__file__).with_name(filename)
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise PipelineError(f"nao foi possivel carregar {filename}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def ovis_layout(inputs: dict[str, Any], params: dict[str, Any], work: Path) -> dict[str, Any]:
    module = _load_module("decalque_dual_ocr", "dual_lmstudio_ocr.py")
    image = Path(inputs["image"])
    prompt = params.get("prompt") or (
        "Analyze this raw scanned page before geometric correction. Preserve reading order, "
        "columns and text. Emit every non-text image as "
        '<img src="images/bbox_X0_Y0_X1_Y1.jpg" /> with coordinates from 0 to 1000.'
    )
    text = module.request_ocr(
        image, params.get("base_url", "http://127.0.0.1:1234/v1"),
        params.get("model", "ovisocr2"), prompt,
    )
    response_path = work / "response.txt"
    response_path.write_text(text + ("" if text.endswith("\n") else "\n"))
    regions = [
        {"kind": "visual", "normalized_bbox": [int(value) for value in match.groups()]}
        for match in IMAGE_TAG.finditer(text)
    ]
    regions_path = work / "regions.json"
    regions_path.write_text(json.dumps({"regions": regions}, indent=2) + "\n")
    return {"response": str(response_path), "regions": str(regions_path)}


def normalize_format(inputs: dict[str, Any], params: dict[str, Any], work: Path) -> dict[str, Any]:
    module = _load_module("decalque_page_format_normalizer", "page_format_normalizer.py")
    if "width_pt" not in params or "height_pt" not in params:
        raise PipelineError("normalize-format requer width_pt e height_pt")
    with Image.open(inputs["image"]) as source:
        normalized, report = module.normalize(
            source.convert("RGB"), float(params["width_pt"]), float(params["height_pt"]),
            str(params.get("side", "unknown")), float(params.get("tolerance", 0.03)),
        )
    image_path, report_path = work / "page.png", work / "transform.json"
    normalized.save(image_path)
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    return {"image": str(image_path), "transform": str(report_path)}


def normalize_page(inputs: dict[str, Any], params: dict[str, Any], work: Path) -> dict[str, Any]:
    module = _load_module("decalque_page_normalizer", "page_geometry_normalizer.py")
    with Image.open(inputs["image"]) as source:
        normalized, report = module.normalize_image(
            source.convert("RGB"), float(params.get("max_angle", 3.0)),
            float(params.get("step", 0.1)),
        )
    image_path, report_path = work / "page.png", work / "transform.json"
    normalized.save(image_path)
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    return {"image": str(image_path), "transform": str(report_path)}


def _rotate_point(x: float, y: float, width: int, height: int, degrees: float) -> tuple[float, float]:
    angle = math.radians(-degrees)
    cx, cy = width / 2, height / 2
    dx, dy = x - cx, y - cy
    return (cx + math.cos(angle) * dx - math.sin(angle) * dy,
            cy + math.sin(angle) * dx + math.cos(angle) * dy)


def transform_regions(inputs: dict[str, Any], params: dict[str, Any], work: Path) -> dict[str, Any]:
    del params
    regions = json.loads(Path(inputs["regions"]).read_text()).get("regions", [])
    transform = json.loads(Path(inputs["transform"]).read_text())
    with Image.open(inputs["image"]) as image:
        width, height = image.size
    degrees = float(transform.get("applied_rotation_deg", 0.0))
    projected = []
    for region in regions:
        x0, y0, x1, y1 = region["normalized_bbox"]
        corners = [_rotate_point(x * width / 1000, y * height / 1000, width, height, degrees)
                   for x, y in ((x0, y0), (x1, y0), (x1, y1), (x0, y1))]
        xs, ys = zip(*corners)
        item = dict(region)
        item["bbox"] = [max(0, round(min(xs))), max(0, round(min(ys))),
                        min(width, round(max(xs))), min(height, round(max(ys)))]
        item["coordinate_space"] = "normalized-page-pixels-ydown"
        projected.append(item)
    path = work / "regions.json"
    path.write_text(json.dumps({"regions": projected}, indent=2) + "\n")
    return {"regions": str(path)}


def segment_ink(inputs: dict[str, Any], params: dict[str, Any], work: Path) -> dict[str, Any]:
    with Image.open(inputs["image"]) as source:
        gray = np.asarray(source.convert("L"))
    threshold = int(params.get("threshold", 190))
    minimum_area = int(params.get("minimum_area", max(8, gray.size * 0.00001)))
    components, count = label(gray < threshold, structure=np.ones((3, 3), dtype=np.uint8))
    regions = []
    for identifier in range(1, count + 1):
        ys, xs = np.where(components == identifier)
        if len(xs) < minimum_area:
            continue
        regions.append({"kind": "ink-component", "bbox": [int(xs.min()), int(ys.min()),
                        int(xs.max()) + 1, int(ys.max()) + 1], "area": int(len(xs))})
    regions.sort(key=lambda item: (item["bbox"][1], item["bbox"][0]))
    path = work / "regions.json"
    path.write_text(json.dumps({"regions": regions}, indent=2) + "\n")
    return {"regions": str(path)}


REGISTRY = {
    "ovis-layout": BlockType(frozenset({"image"}), frozenset({"response", "regions"}), ovis_layout),
    "normalize-format": BlockType(frozenset({"image"}), frozenset({"image", "transform"}),
                                  normalize_format),
    "normalize-page": BlockType(frozenset({"image"}), frozenset({"image", "transform"}), normalize_page),
    "transform-regions": BlockType(frozenset({"image", "regions", "transform"}),
                                   frozenset({"regions"}), transform_regions),
    "segment-ink": BlockType(frozenset({"image"}), frozenset({"regions"}), segment_ink),
}


def validate(config: dict[str, Any], registry: dict[str, BlockType] = REGISTRY) -> list[str]:
    if config.get("version") != 1:
        raise PipelineError("version deve ser 1")
    blocks = config.get("blocks")
    if not isinstance(blocks, list) or not blocks:
        raise PipelineError("blocks deve ser uma lista nao vazia")
    by_id: dict[str, dict[str, Any]] = {}
    for block in blocks:
        identifier = block.get("id")
        if not isinstance(identifier, str) or not identifier or identifier in by_id:
            raise PipelineError(f"id de bloco invalido ou repetido: {identifier!r}")
        if block.get("type") not in registry:
            raise PipelineError(f"tipo de bloco desconhecido: {block.get('type')!r}")
        by_id[identifier] = block
    inputs = config.get("inputs", {})
    dependencies: dict[str, set[str]] = {identifier: set() for identifier in by_id}
    for identifier, block in by_id.items():
        kind = registry[block["type"]]
        bindings = block.get("inputs", {})
        if set(bindings) != set(kind.inputs):
            raise PipelineError(f"entradas de {identifier}: esperado {sorted(kind.inputs)}")
        for port, reference in bindings.items():
            if not isinstance(reference, str) or ":" not in reference and "." not in reference:
                raise PipelineError(f"referencia invalida em {identifier}.{port}")
            if reference.startswith("input:"):
                if reference[6:] not in inputs:
                    raise PipelineError(f"entrada global inexistente: {reference}")
                continue
            source, output = reference.rsplit(".", 1)
            if source not in by_id:
                raise PipelineError(f"bloco de origem inexistente: {source}")
            source_kind = registry[by_id[source]["type"]]
            if output not in source_kind.outputs:
                raise PipelineError(f"saida inexistente: {reference}")
            dependencies[identifier].add(source)
    order: list[str] = []
    remaining = set(by_id)
    while remaining:
        ready = sorted(node for node in remaining if dependencies[node] <= set(order))
        if not ready:
            raise PipelineError("a cadeia contem ciclo")
        order.extend(ready)
        remaining.difference_update(ready)
    return order


def run(config: dict[str, Any], output: Path,
        registry: dict[str, BlockType] = REGISTRY) -> dict[str, Any]:
    order = validate(config, registry)
    output.mkdir(parents=True, exist_ok=True)
    by_id = {block["id"]: block for block in config["blocks"]}
    artifacts: dict[str, Any] = {f"input:{key}": value for key, value in config["inputs"].items()}
    records = []
    for identifier in order:
        block = by_id[identifier]
        bindings = {port: artifacts[reference] for port, reference in block["inputs"].items()}
        work = output / identifier
        work.mkdir(parents=True, exist_ok=False)
        started = time.monotonic()
        produced = registry[block["type"]].execute(bindings, block.get("params", {}), work)
        expected = registry[block["type"]].outputs
        if set(produced) != set(expected):
            raise PipelineError(f"saidas de {identifier}: esperado {sorted(expected)}")
        for port, value in produced.items():
            artifacts[f"{identifier}.{port}"] = value
        records.append({"id": identifier, "type": block["type"], "inputs": block["inputs"],
                        "outputs": produced, "duration_ms": round((time.monotonic() - started) * 1000)})
    manifest = {"version": 1, "status": "completed", "order": order, "blocks": records,
                "artifacts": artifacts}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pipeline", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    try:
        config = json.loads(args.pipeline.read_text())
        order = validate(config)
        if args.dry_run:
            print(json.dumps({"status": "valid", "order": order}, indent=2))
        else:
            print(json.dumps(run(config, args.output), indent=2))
        return 0
    except Exception as error:
        print(f"block-ocr-pipeline: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
