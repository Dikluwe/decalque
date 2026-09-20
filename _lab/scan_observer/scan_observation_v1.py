#!/usr/bin/env python3
# Crystalline Lineage
# @prompt _lab/scan_observer/specs/scan-observation-exporter.md
"""Build deterministic Decalque ScanObservation v1 artifacts from Paddle lines."""

from __future__ import annotations

import hashlib
import json
import math
import os
import tempfile
from pathlib import Path
from typing import Any

from PIL import Image


PRODUCER_NAME = "decalque-paddle-line-exporter"
PRODUCER_VERSION = "1.0.0"


def _canonical_sha256(value: Any) -> str:
    encoded = json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def _detect_media_type(data: bytes) -> str:
    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        return "image/png"
    if data.startswith(b"\xff\xd8"):
        return "image/jpeg"
    if data.startswith((b"P2", b"P5")) and len(data) > 2 and data[2:3].isspace():
        return "image/x-portable-graymap"
    raise ValueError("raster deve ser PNG, JPEG ou PGM")


def _unknown(reason: str, detail: str, evidence: list[str] | None = None) -> dict[str, Any]:
    return {
        "status": "unknown",
        "reason": reason,
        "evidence": list(evidence or []),
        "detail": detail,
    }


def _unknown_confidence(reason: str) -> dict[str, Any]:
    return {"status": "unknown", "reason": reason}


def _known(value: Any, basis: str, evidence: list[str],
           confidence: dict[str, Any]) -> dict[str, Any]:
    return {
        "status": "known",
        "value": value,
        "basis": basis,
        "evidence": evidence,
        "confidence": confidence,
    }


def _known_text(text: Any, score: Any) -> dict[str, Any]:
    if not isinstance(text, str) or not text:
        return _unknown("not-observed", "Paddle did not emit non-empty line text", ["prov-text"])
    if isinstance(score, bool) or not isinstance(score, (int, float)):
        confidence = _unknown_confidence("not-observed")
    elif not math.isfinite(float(score)) or not 0.0 <= float(score) <= 1.0:
        confidence = _unknown_confidence("invalid")
    else:
        confidence = {
            "status": "known",
            "value": float(score),
            "semantics": "paddle-recognition-score",
        }
    return _known(text, "inferred", ["prov-text"], confidence)


def _geometry_claims(polygon: Any, width: int, height: int,
                     diagnostics: list[dict[str, str]], unit_id: str,
                     provider_bbox: Any) -> dict[str, Any]:
    try:
        if not isinstance(polygon, list) or len(polygon) < 3:
            raise ValueError("polygon must contain at least three points")
        points: list[list[float]] = []
        for point in polygon:
            if not isinstance(point, (list, tuple)) or len(point) != 2:
                raise ValueError("polygon point must contain x and y")
            x, y = point
            if isinstance(x, bool) or isinstance(y, bool):
                raise ValueError("boolean coordinate")
            x, y = float(x), float(y)
            if not math.isfinite(x) or not math.isfinite(y):
                raise ValueError("non-finite coordinate")
            if not 0.0 <= x <= width or not 0.0 <= y <= height:
                raise ValueError("coordinate outside raster")
            points.append([x, y])
        if len({tuple(point) for point in points}) < 3:
            raise ValueError("degenerate polygon")
        xs = [point[0] for point in points]
        ys = [point[1] for point in points]
        x0, y0, x1, y1 = min(xs), min(ys), max(xs), max(ys)
        if x0 >= x1 or y0 >= y1:
            raise ValueError("degenerate polygon envelope")
    except (TypeError, ValueError, OverflowError) as error:
        diagnostics.append({
            "level": "warning",
            "code": "invalid-line-geometry",
            "detail": f"{unit_id}: {error}",
        })
        unknown = _unknown("invalid", "Paddle line polygon is not valid raster geometry",
                           ["prov-geometry"])
        return {
            "bbox": dict(unknown),
            "polygon": dict(unknown),
            "baseline": _unknown("not-observed", "Paddle did not emit a baseline"),
        }

    envelope = {
        "frame_id": "scan-px",
        "x0": x0,
        "y0": y0,
        "x1": x1,
        "y1": y1,
    }
    if isinstance(provider_bbox, (list, tuple)) and len(provider_bbox) == 4:
        try:
            supplied = [float(value) for value in provider_bbox]
        except (TypeError, ValueError, OverflowError):
            supplied = []
        if supplied and supplied != [x0, y0, x1, y1]:
            diagnostics.append({
                "level": "information",
                "code": "provider-bbox-disagrees-with-polygon",
                "detail": f"{unit_id}: bbox was derived from the polygon envelope",
            })

    return {
        "bbox": _known(
            envelope,
            "derived",
            ["prov-bbox"],
            _unknown_confidence("not-observed"),
        ),
        "polygon": _known(
            {"frame_id": "scan-px", "points": points},
            "inferred",
            ["prov-geometry"],
            _unknown_confidence("not-observed"),
        ),
        "baseline": _unknown("not-observed", "Paddle did not emit a baseline"),
    }


def build_observation(
    raster_path: str | Path,
    normalized_page: dict[str, Any],
    *,
    page_index: int,
    run_id: str,
    lang: str,
    ocr_version: str,
    device: str,
    paddle_version: str = "runtime-unspecified",
) -> dict[str, Any]:
    """Translate one normalized Paddle line page into strict ScanObservation v1."""
    if isinstance(page_index, bool) or not isinstance(page_index, int) or not 0 <= page_index <= 2**32 - 1:
        raise ValueError("page_index deve ser um inteiro zero-based valido")
    for name, value in (("run_id", run_id), ("lang", lang),
                        ("ocr_version", ocr_version), ("device", device),
                        ("paddle_version", paddle_version)):
        if not isinstance(value, str) or not value:
            raise ValueError(f"{name} deve ser uma string nao vazia")
    if not isinstance(normalized_page, dict) or not isinstance(normalized_page.get("lines"), list):
        raise ValueError("normalized_page.lines deve ser uma lista")

    raster_path = Path(raster_path)
    raster_bytes = raster_path.read_bytes()
    media_type = _detect_media_type(raster_bytes)
    digest = hashlib.sha256(raster_bytes).hexdigest()
    with Image.open(raster_path) as image:
        image.load()
        width, height = image.size
    if width <= 0 or height <= 0:
        raise ValueError("raster deve ter dimensoes positivas")
    artifact_id = f"raster-{digest[:16]}"

    provider_parameters = {
        "device": device,
        "enable_mkldnn": False,
        "lang": lang,
        "ocr_version": ocr_version,
        "paddle_version": paddle_version,
        "return_word_box": False,
    }
    provenance = [
        {
            "id": "prov-geometry",
            "stage": "layout-detection",
            "tool_name": "paddleocr",
            "tool_version": paddle_version,
            "model_identifier": ocr_version,
            "method": "text-line-polygon-detection",
            "parameters_sha256": _canonical_sha256(
                {"stage": "geometry", **provider_parameters}
            ),
            "input_artifact_ids": [artifact_id],
            "parent_provenance_ids": [],
        },
        {
            "id": "prov-text",
            "stage": "text-recognition",
            "tool_name": "paddleocr",
            "tool_version": paddle_version,
            "model_identifier": ocr_version,
            "method": "text-line-recognition",
            "parameters_sha256": _canonical_sha256(
                {"stage": "text", **provider_parameters}
            ),
            "input_artifact_ids": [artifact_id],
            "parent_provenance_ids": [],
        },
        {
            "id": "prov-bbox",
            "stage": "segmentation",
            "tool_name": PRODUCER_NAME,
            "tool_version": PRODUCER_VERSION,
            "model_identifier": "none",
            "method": "deterministic-polygon-envelope",
            "parameters_sha256": _canonical_sha256(
                {"method": "deterministic-polygon-envelope", "version": 1}
            ),
            "input_artifact_ids": [],
            "parent_provenance_ids": ["prov-geometry"],
        },
    ]

    diagnostics: list[dict[str, str]] = []
    units = []
    for index, line in enumerate(normalized_page["lines"]):
        if not isinstance(line, dict):
            raise ValueError(f"lines[{index}] deve ser um objeto")
        unit_id = f"line-{index + 1:06d}"
        units.append({
            "id": unit_id,
            "kind": "line",
            "reading_order": _known(
                index,
                "inferred",
                ["prov-geometry"],
                _unknown_confidence("not-observed"),
            ),
            "text": _known_text(line.get("text"), line.get("recognition_confidence")),
            "span_in_parent": _unknown("not-observed", "top-level line has no parent"),
            "geometry": _geometry_claims(
                line.get("polygon"), width, height, diagnostics, unit_id, line.get("bbox")
            ),
        })

    return {
        "schema": "decalque.scan-observation",
        "schema_version": 1,
        "source": {
            "page_index": page_index,
            "raster": {
                "artifact_id": artifact_id,
                "sha256": digest,
                "media_type": media_type,
                "width_px": width,
                "height_px": height,
            },
        },
        "producer": {
            "name": PRODUCER_NAME,
            "version": PRODUCER_VERSION,
            "run_id": run_id,
        },
        "raster_frame": {
            "id": "scan-px",
            "unit": "px",
            "origin": "top-left",
            "x_direction": "right",
            "y_direction": "down",
            "coordinate_basis": "pixel-edges",
            "extent": [width, height],
        },
        "page_mapping": _unknown(
            "not-observed",
            "the Paddle line producer has no physical source-page calibration",
        ),
        "provenance": provenance,
        "units": units,
        "diagnostics": diagnostics,
    }


def serialize_observation(observation: dict[str, Any]) -> str:
    """Serialize without locale, timestamps, paths or nondeterministic key ordering."""
    return json.dumps(
        observation,
        ensure_ascii=False,
        separators=(",", ":"),
        allow_nan=False,
    ) + "\n"


def write_observation_atomic(path: str | Path, observation: dict[str, Any]) -> None:
    path = Path(path)
    payload = serialize_observation(observation)
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8", newline="\n") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary_name, path)
    except BaseException:
        try:
            os.unlink(temporary_name)
        except FileNotFoundError:
            pass
        raise
