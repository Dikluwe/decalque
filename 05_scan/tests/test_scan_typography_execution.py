import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parents[2]
SCAN = ROOT / "05_scan"
FIXTURE = ROOT / "03_infra" / "tests" / "fixtures" / "typst.pdf"


class ScanTypographyExecutionTests(unittest.TestCase):
    def run_with_json(self, script, values, *arguments):
        with tempfile.TemporaryDirectory() as directory:
            paths = []
            for index, value in enumerate(values):
                path = pathlib.Path(directory) / f"input-{index}.json"
                path.write_text(json.dumps(value), encoding="utf-8")
                paths.append(path)
            return subprocess.run(
                [sys.executable, str(SCAN / script), *map(str, paths), *map(str, arguments)],
                cwd=ROOT, capture_output=True, text=True, check=False,
            )

    def test_candidate_raster_cli_measures_real_pdf_ink(self):
        page = [{
            "width": 945, "height": 567,
            "regions": [{"detected_lines": [{"word_segments": [{"text": "O"}]}]}],
        }]
        catalog = {
            "page": {"width_pt": 566.9291381835938, "height_pt": 340.157470703125},
            "glyphs": [{
                "text": "O", "position": [221.58056640625, 57.99969482421875],
                "advance": 17.52, "font_size_pt": 24, "font_ref": "f0",
            }],
        }
        result = self.run_with_json(
            "candidate_raster_profile.py", [page, catalog], FIXTURE,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        profile = json.loads(result.stdout)[0]["regions"][0]["detected_lines"][0]["word_segments"][0]["candidate_typographic_profile"]
        self.assertEqual(profile["status"], "observed")
        self.assertIsNotNone(profile["ink_shape"])

    def test_candidate_raster_cli_reports_renderer_failure(self):
        page = [{"width": 100, "height": 100, "regions": []}]
        catalog = {"page": {"width_pt": 100, "height_pt": 100}, "glyphs": []}
        result = self.run_with_json(
            "candidate_raster_profile.py", [page, catalog], "missing-candidate.pdf",
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("candidate-raster-profile:", result.stderr)

    def test_word_compare_cli_emits_typographic_violation_witness(self):
        shape = {
            "version": 1, "bins": 2, "centroid_x": 0.5, "centroid_y": 0.5,
            "horizontal_projection": [0.2, 0.2], "vertical_projection": [0.2, 0.2],
        }
        segment = {
            "text": "casa", "bbox_pt": [10, 5, 30, 15],
            "baseline_y_pt": 20, "baseline_confidence": 1,
            "typographic_profile": {"status": "observed", "ink_shape": {**shape, "density": 0.2}},
            "candidate_typographic_profile": {"status": "observed", "ink_shape": {
                **shape, "density": 0.8,
                "horizontal_projection": [0.8, 0.8],
                "vertical_projection": [0.8, 0.8],
            }},
        }
        page = [{"regions": [{"detected_lines": [{"id": 1, "word_segments": [segment]}]}]}]
        catalog = {"glyphs": [
            {"text": character, "position": [10 + index * 5, 20], "advance": 5,
             "font_size_pt": 10, "font_ref": "F1"}
            for index, character in enumerate("casa")
        ]}
        result = self.run_with_json("scan_word_compare.py", [page, catalog])
        self.assertEqual(result.returncode, 0, result.stderr)
        word = json.loads(result.stdout)["words"][0]
        self.assertEqual(word["typography_status"], "violated")
        self.assertGreater(word["typographic_shape_distance"], word["typographic_shape_tolerance"])


if __name__ == "__main__":
    unittest.main()
