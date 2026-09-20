# Crystalline Lineage
# @prompt _lab/scan_observer/specs/page-zone-consensus.md
# @updated 2026-09-16

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw


SCRIPT = pathlib.Path(__file__).parents[1] / "page_zone_marker.py"


class PageZoneMarkerExecutionTests(unittest.TestCase):
    def test_marks_regions_quadrants_lines_anchors_and_provider_consensus(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image_path = root / "page.png"
            image = Image.new("RGB", (200, 160), "white")
            draw = ImageDraw.Draw(image)
            for y, spans in (
                (20, [(20, 40), (48, 88), (100, 180)]),
                (38, [(20, 55), (64, 120), (130, 180)]),
                (100, [(70, 95), (104, 145), (153, 190)]),
                (118, [(70, 110), (119, 150), (160, 190)]),
            ):
                for x0, x1 in spans:
                    draw.rectangle((x0, y, x1, y + 7), fill="black")
            image.save(image_path)
            ocr_path = root / "ocr.json"
            ocr_path.write_text(json.dumps({
                "providers": {
                    "layout": {"model": "ovisocr2", "text": "Alpha middle omega\nBeta middle end\n\nGamma middle last\nDelta middle final"},
                    "text_witness": {"model": "paddleocr-vl", "text": "Alpha middle omega\nBeta middle end\nGamma middle last\nDelta middle final"},
                }
            }), encoding="utf-8")
            output_json = root / "zones.json"
            overlay = root / "zones.png"

            process = subprocess.run([
                sys.executable, str(SCRIPT), str(image_path), str(ocr_path),
                "--output", str(output_json), "--overlay", str(overlay),
            ], capture_output=True, text=True, check=False)

            self.assertEqual(process.returncode, 0, process.stderr)
            result = json.loads(output_json.read_text(encoding="utf-8"))
            self.assertEqual(len(result["quadrants"]), 4)
            self.assertEqual(len(result["regions"]), 2)
            self.assertEqual(result["regions"][0]["consensus_status"], "confirmed")
            self.assertEqual(result["regions"][1]["quadrants"], ["Q3", "Q4"])
            line = result["regions"][0]["lines"][0]
            self.assertEqual(line["text"], "Alpha middle omega")
            self.assertEqual(line["margin_anchor_status"], "observed")
            self.assertEqual(line["left"]["text"], "Alpha")
            self.assertEqual(line["right"]["text"], "omega")
            self.assertEqual(result["providers"]["got_ocr2"], "unavailable")
            self.assertTrue(overlay.is_file())
            self.assertGreater(overlay.stat().st_size, 0)

    def test_line_count_mismatch_is_unknown_not_silently_zipped(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image_path = root / "page.png"
            image = Image.new("RGB", (100, 60), "white")
            ImageDraw.Draw(image).rectangle((10, 20, 90, 28), fill="black")
            image.save(image_path)
            ocr_path = root / "ocr.json"
            ocr_path.write_text(json.dumps({"providers": {
                "layout": {"model": "ovisocr2", "text": "one\ntwo"},
                "text_witness": {"model": "paddleocr-vl", "text": "one\ntwo"},
            }}), encoding="utf-8")

            process = subprocess.run([
                sys.executable, str(SCRIPT), str(image_path), str(ocr_path),
                "--output", str(root / "zones.json"), "--overlay", str(root / "zones.png"),
            ], capture_output=True, text=True, check=False)

            self.assertNotEqual(process.returncode, 0)
            self.assertIn("contagem", process.stderr)
            self.assertFalse((root / "zones.json").exists())


if __name__ == "__main__":
    unittest.main()
