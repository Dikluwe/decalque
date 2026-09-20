import hashlib
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest
import zlib

from PIL import Image


LAB = pathlib.Path(__file__).parents[1]
ROOT = pathlib.Path(__file__).parents[3]


def png_with_truncated_idat_and_valid_container(data: bytes) -> bytes:
    index = 8
    while index + 12 <= len(data):
        length = int.from_bytes(data[index:index + 4], "big")
        end = index + 12 + length
        if data[index + 4:index + 8] == b"IDAT":
            retained = max(1, length // 2)
            if retained >= length:
                raise AssertionError("IDAT fixture is too short")
            payload = data[index + 8:index + 8 + retained]
            kind = b"IDAT"
            crc = zlib.crc32(kind + payload).to_bytes(4, "big")
            return (data[:index] + retained.to_bytes(4, "big") + kind
                    + payload + crc + data[end:])
        index = end
    raise AssertionError("PNG fixture must contain IDAT")


def load_module(name: str, filename: str):
    path = LAB / filename
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


class ScanObservationV1Tests(unittest.TestCase):
    def setUp(self):
        self.module = load_module("scan_observation_v1", "scan_observation_v1.py")
        self.temporary = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary.name)
        self.raster = self.root / "page.png"
        Image.new("L", (120, 80), 255).save(self.raster)

    def tearDown(self):
        self.temporary.cleanup()

    def build(self, page=None, **metadata):
        if page is None:
            page = {
                "lines": [
                    {
                        "id": 7,
                        "text": "Straße  A... e\u0301 😀",
                        "recognition_confidence": 0.42,
                        "polygon": [[10, 20], [90, 18], [92, 32], [12, 34]],
                        "bbox": [1, 2, 3, 4],
                    }
                ]
            }
        values = {
            "page_index": 4,
            "run_id": "fixed-run",
            "lang": "pt",
            "ocr_version": "PP-OCRv5",
            "device": "cpu",
        }
        values.update(metadata)
        return self.module.build_observation(self.raster, page, **values)

    def test_emits_strict_identity_exact_text_and_only_line_units(self):
        observation = self.build()

        self.assertEqual(observation["schema"], "decalque.scan-observation")
        self.assertEqual(observation["schema_version"], 1)
        self.assertEqual(observation["source"]["page_index"], 4)
        raster = observation["source"]["raster"]
        self.assertEqual(raster["sha256"], hashlib.sha256(self.raster.read_bytes()).hexdigest())
        self.assertEqual(raster["media_type"], "image/png")
        self.assertEqual((raster["width_px"], raster["height_px"]), (120, 80))
        self.assertTrue(raster["artifact_id"].endswith(raster["sha256"][:16]))
        self.assertEqual(observation["page_mapping"]["status"], "unknown")

        self.assertEqual(len(observation["units"]), 1)
        unit = observation["units"][0]
        self.assertEqual(unit["id"], "line-000001")
        self.assertEqual(unit["kind"], "line")
        self.assertNotIn("parent_id", unit)
        self.assertEqual(unit["text"]["value"], "Straße  A... e\u0301 😀")
        self.assertEqual(unit["text"]["confidence"]["value"], 0.42)
        self.assertFalse(any(item["kind"] in {"word", "glyph"} for item in observation["units"]))

    def test_bbox_is_polygon_envelope_and_text_score_is_not_geometry_confidence(self):
        unit = self.build()["units"][0]

        self.assertEqual(
            unit["geometry"]["bbox"]["value"],
            {"frame_id": "scan-px", "x0": 10.0, "y0": 18.0, "x1": 92.0, "y1": 34.0},
        )
        self.assertNotEqual(unit["geometry"]["bbox"]["value"],
                            {"frame_id": "scan-px", "x0": 1, "y0": 2, "x1": 3, "y1": 4})
        self.assertEqual(unit["geometry"]["bbox"]["confidence"]["status"], "unknown")
        self.assertEqual(unit["geometry"]["polygon"]["confidence"]["status"], "unknown")

    def test_preserves_empty_punctuation_and_low_confidence_lines(self):
        page = {
            "lines": [
                {"text": "", "recognition_confidence": None,
                 "polygon": [[1, 1], [20, 1], [20, 10], [1, 10]], "bbox": [1, 1, 20, 10]},
                {"text": "...?!", "recognition_confidence": 0.001,
                 "polygon": [[1, 20], [20, 20], [20, 30], [1, 30]], "bbox": [1, 20, 20, 30]},
            ]
        }
        units = self.build(page)["units"]

        self.assertEqual(len(units), 2)
        self.assertEqual(units[0]["text"]["status"], "unknown")
        self.assertEqual(units[1]["text"]["value"], "...?!")
        self.assertEqual(units[1]["text"]["confidence"]["value"], 0.001)

    def test_invalid_geometry_becomes_unknown_without_clamp_or_line_loss(self):
        page = {
            "lines": [
                {"text": "fora", "recognition_confidence": float("nan"),
                 "polygon": [[-1, 1], [121, 1], [121, 10], [-1, 10]],
                 "bbox": [0, 1, 120, 10]},
            ]
        }
        observation = self.build(page)
        unit = observation["units"][0]

        self.assertEqual(unit["text"]["status"], "known")
        self.assertEqual(unit["text"]["confidence"]["status"], "unknown")
        self.assertEqual(unit["geometry"]["bbox"]["status"], "unknown")
        self.assertEqual(unit["geometry"]["polygon"]["status"], "unknown")
        self.assertIn("invalid-line-geometry",
                      {item["code"] for item in observation["diagnostics"]})

    def test_serialization_is_deterministic_and_parameter_hashes_track_configuration(self):
        first = self.build()
        second = self.build()
        self.assertEqual(
            self.module.serialize_observation(first),
            self.module.serialize_observation(second),
        )
        changed = self.build(lang="en")
        first_hashes = [item["parameters_sha256"] for item in first["provenance"]]
        changed_hashes = [item["parameters_sha256"] for item in changed["provenance"]]
        self.assertNotEqual(first_hashes, changed_hashes)
        for item in first["provenance"]:
            if not item["parent_provenance_ids"]:
                self.assertEqual(item["input_artifact_ids"],
                                 [first["source"]["raster"]["artifact_id"]])

    def test_cli_rejects_candidate_option_before_provider_or_output(self):
        output = self.root / "observation.json"
        result = subprocess.run(
            [sys.executable, str(LAB / "paddle_scan_observation.py"), str(self.raster),
             "--output", str(output), "--page-index", "4", "--run-id", "fixed-run",
             "--candidate", "candidate.pdf"],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertFalse(output.exists())

    def test_rejects_truncated_png_and_pgm_pixel_payloads(self):
        page = {"lines": []}
        self.raster.write_bytes(
            png_with_truncated_idat_and_valid_container(self.raster.read_bytes())
        )
        with self.assertRaises(Exception):
            self.build(page)

        self.raster.write_bytes(b"P5\n2 2\n255\n\x00")
        with self.assertRaises(Exception):
            self.build(page)


class OvisSidecarTests(unittest.TestCase):
    def test_preserves_lossless_ordered_response_without_promoting_claims(self):
        block = load_module("block_ocr_pipeline_sidecar", "block_ocr_pipeline.py")
        raw = "# Título\nTexto  com espaços\n<img src=\"images/bbox_10_20_30_40.jpg\" />\nFim"

        class Stub:
            @staticmethod
            def request_ocr(*_args, **_kwargs):
                return raw

        original = block._load_module
        block._load_module = lambda *_args: Stub
        try:
            with tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                image = root / "page.png"
                Image.new("RGB", (10, 10), "white").save(image)
                work = root / "work"
                work.mkdir()
                produced = block.ovis_layout({"image": str(image)}, {}, work)
                sidecar = json.loads(pathlib.Path(produced["regions"]).read_text())
        finally:
            block._load_module = original

        rebuilt = "".join(
            item["text"] if item["kind"] == "text" else item["markup"]
            for item in sidecar["segments"]
        )
        self.assertEqual(rebuilt, raw)
        self.assertEqual(sidecar["response_sha256"], hashlib.sha256(raw.encode()).hexdigest())
        self.assertEqual(len(sidecar["regions"]), 1)
        self.assertNotIn("schema", sidecar)


if __name__ == "__main__":
    unittest.main()
