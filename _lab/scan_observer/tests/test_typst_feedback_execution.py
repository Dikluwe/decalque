import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image


SCAN = pathlib.Path(__file__).parents[1]
MATERIALIZER = SCAN / "typst_page_materializer.py"
OPTIMIZER = SCAN / "typst_feedback_optimizer.py"


class TypstFeedbackExecutionTests(unittest.TestCase):
    def test_real_cycles_recover_known_horizontal_and_vertical_shift(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            Image.new("RGB", (30, 20), "black").save(root / "mark.png")
            exact = {
                "schema_version": 1,
                "page": {"width_pt": 200, "height_pt": 120, "source_width_px": 400, "source_height_px": 240},
                "regions": [
                    {"kind": "image", "path": "mark.png", "bbox_px": [40, 30, 100, 70]},
                    {"kind": "text", "text": "Feedback", "bbox_px": [140, 100, 350, 160],
                     "font": {"family": "Liberation Serif", "size_pt": 18}},
                ],
            }
            exact_path = root / "exact.json"
            exact_typ = root / "exact.typ"
            source_pdf = root / "source.pdf"
            exact_path.write_text(json.dumps(exact))
            materialized = subprocess.run([sys.executable, str(MATERIALIZER), str(exact_path), str(exact_typ)],
                                          capture_output=True, text=True)
            self.assertEqual(materialized.returncode, 0, materialized.stderr)
            compiled = subprocess.run(["typst", "compile", "--root", "/", str(exact_typ), str(source_pdf)],
                                      capture_output=True, text=True)
            self.assertEqual(compiled.returncode, 0, compiled.stderr)
            shifted = json.loads(json.dumps(exact))
            for region in shifted["regions"]:
                x0, y0, x1, y1 = region["bbox_px"]
                region["bbox_px"] = [x0 + 12, y0 - 8, x1 + 12, y1 - 8]
            shifted_path = root / "shifted.json"
            shifted_path.write_text(json.dumps(shifted))
            process = subprocess.run(
                [sys.executable, str(OPTIMIZER), str(shifted_path), str(source_pdf),
                 "--output-dir", str(root / "optimized"), "--dpi", "144",
                 "--horizontal-threshold", "0.0001", "--vertical-threshold", "0.0001",
                 "--max-cycles", "4", "--max-shift-px", "20"],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertEqual(output["status"], "converged")
            self.assertGreaterEqual(output["cycles_executed"], 2)
            first, last = output["history"][0], output["history"][-1]
            self.assertLess(last["horizontal_line_error"], first["horizontal_line_error"])
            self.assertLess(last["vertical_line_error"], first["vertical_line_error"])
            self.assertTrue(pathlib.Path(output["pdf"]).exists())

    def test_zonal_cycles_fix_internal_spacing_when_global_center_is_already_right(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            Image.new("RGB", (40, 16), "black").save(root / "bar.png")
            exact = {
                "schema_version": 1,
                "page": {"width_pt": 200, "height_pt": 160, "source_width_px": 400, "source_height_px": 320},
                "regions": [
                    {"kind": "image", "path": "bar.png", "bbox_px": [100, y, 180, y + 32]}
                    for y in (40, 140, 240)
                ],
            }
            exact_path, exact_typ, source_pdf = root / "exact.json", root / "exact.typ", root / "source.pdf"
            exact_path.write_text(json.dumps(exact))
            self.assertEqual(subprocess.run([sys.executable, str(MATERIALIZER), str(exact_path), str(exact_typ)]).returncode, 0)
            self.assertEqual(subprocess.run(["typst", "compile", "--root", "/", str(exact_typ), str(source_pdf)]).returncode, 0)
            disturbed = json.loads(json.dumps(exact))
            for region, delta in zip(disturbed["regions"], (-16, 0, 16)):
                region["bbox_px"][1] += delta
                region["bbox_px"][3] += delta
            disturbed_path = root / "disturbed.json"
            disturbed_path.write_text(json.dumps(disturbed))
            process = subprocess.run(
                [sys.executable, str(OPTIMIZER), str(disturbed_path), str(source_pdf),
                 "--output-dir", str(root / "optimized"), "--dpi", "144",
                 "--horizontal-threshold", "0.0001", "--vertical-threshold", "0.0001",
                 "--zone-position-threshold", "1", "--zone-gap-threshold", "1",
                 "--max-cycles", "4"], capture_output=True, text=True, check=False,
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertEqual(output["status"], "converged")
            first_gaps = output["history"][0]["zones"]["gaps"]
            last_gaps = output["history"][-1]["zones"]["gaps"]
            self.assertGreater(max(abs(gap["gap_delta_px"]) for gap in first_gaps), 1)
            self.assertLessEqual(max(abs(gap["gap_delta_px"]) for gap in last_gaps), 1)


if __name__ == "__main__":
    unittest.main()
