import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw, ImageFont


SCRIPT = pathlib.Path(__file__).parents[1] / "local_font_discovery.py"
SERIF = pathlib.Path("/usr/share/fonts/truetype/liberation2/LiberationSerif-Regular.ttf")
SANS = pathlib.Path("/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf")
if not SERIF.exists():
    SERIF = pathlib.Path("/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf")
if not SANS.exists():
    SANS = pathlib.Path("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf")


def evidence(root: pathlib.Path, font: pathlib.Path = SERIF) -> pathlib.Path:
    image = Image.new("L", (900, 260), 255)
    ImageDraw.Draw(image).text((20, 20), "Algorithm", font=ImageFont.truetype(str(font), 180), fill=0)
    image.save(root / "sample.png")
    path = root / "evidence.json"
    path.write_text(json.dumps({"samples": [{
        "id": "algorithm", "path": "sample.png", "text": "Algorithm", "weight": 1,
        "dimensions": ["family", "weight"], "variants": ["regular"],
    }]}))
    return path


class LocalFontDiscoveryExecutionTests(unittest.TestCase):
    def run_discovery(self, *arguments: str):
        return subprocess.run([sys.executable, str(SCRIPT), *arguments], capture_output=True,
                              text=True, check=False, timeout=10)

    def test_exact_font_recovers_render_size_and_reports_quadratim_metrics(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fonts = root / "fonts"
            fonts.mkdir()
            shutil.copy2(SERIF, fonts / "serif.ttf")
            process = self.run_discovery("--evidence", str(evidence(root)), "--font-dir",
                                         str(fonts), "--pt-per-px", "0.4")
            self.assertEqual(process.returncode, 0, process.stderr)
            winner = json.loads(process.stdout)["ranked"][0]
            measurement = winner["evidence"][0]["geometry"]
            self.assertAlmostEqual(measurement["estimated_size_px"], 180, delta=1.5)
            self.assertAlmostEqual(measurement["estimated_size_pt"], 72, delta=0.6)
            self.assertAlmostEqual(measurement["estimated_tracking_em"], 0, delta=0.003)
            self.assertAlmostEqual(measurement["width_scale_without_tracking"], 1, delta=0.003)
            self.assertIn("bbox_px", measurement["observed_ink"])
            self.assertGreater(winner["units_per_em"], 0)
            self.assertGreater(winner["m_advance_em"], 0)

    def test_recursively_ranks_exact_font_and_deduplicates_content(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fonts = root / "fonts"
            (fonts / "nested").mkdir(parents=True)
            shutil.copy2(SERIF, fonts / "serif-copy.ttf")
            shutil.copy2(SERIF, fonts / "nested/duplicate.ttf")
            shutil.copy2(SANS, fonts / "sans.ttf")
            process = self.run_discovery("--evidence", str(evidence(root)), "--font-dir",
                                         str(fonts), "--threshold", "0.9", "--limit", "10")
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertEqual(output["status"], "matched")
            self.assertEqual(output["ranked"][0]["family"], "Liberation Serif")
            self.assertGreaterEqual(output["ranked"][0]["overall_score"], 0.99)
            hashes = [item["sha256"] for item in output["ranked"]]
            self.assertEqual(len(hashes), len(set(hashes)))
            self.assertEqual(len(output["ranked"]), 2)
            self.assertEqual(output["network_access"], "none")

    def test_score_below_threshold_requests_external_fallback_without_network(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fonts = root / "fonts"
            fonts.mkdir()
            shutil.copy2(SANS, fonts / "sans.ttf")
            process = self.run_discovery("--evidence", str(evidence(root)), "--font-dir",
                                         str(fonts), "--threshold", "0.99")
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertEqual(output["status"], "fallback_required")
            self.assertTrue(output["ranked"])
            self.assertLess(output["ranked"][0]["overall_score"], 0.99)

    def test_symlinked_font_is_read_and_keeps_explicit_origin(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fonts = root / "fonts"
            fonts.mkdir()
            (fonts / "linked.ttf").symlink_to(SERIF)
            process = self.run_discovery("--evidence", str(evidence(root)), "--font-dir",
                                         str(fonts), "--threshold", "0.9")
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertEqual(output["status"], "matched")
            self.assertEqual(output["ranked"][0]["origin"], f"explicit:{fonts.resolve()}")

    def test_empty_directory_is_unknown_and_invalid_font_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fonts = root / "fonts"
            fonts.mkdir()
            (fonts / "broken.ttf").write_bytes(b"not-a-font")
            process = self.run_discovery("--evidence", str(evidence(root)), "--font-dir",
                                         str(fonts))
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertEqual(output["status"], "unknown")
            self.assertEqual(output["ranked"], [])
            self.assertEqual(output["rejected"][0]["reason"], "font_unusable")
            self.assertNotIn("not-a-font", process.stdout + process.stderr)

    def test_invalid_threshold_is_process_error(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            process = self.run_discovery("--evidence", str(evidence(root)), "--font-dir",
                                         str(root), "--threshold", "1.1")
            self.assertEqual(process.returncode, 2)
            self.assertIn("threshold", process.stderr)

    def test_invalid_point_scale_is_process_error(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            process = self.run_discovery("--evidence", str(evidence(root)), "--font-dir",
                                         str(root), "--pt-per-px", "0")
            self.assertEqual(process.returncode, 2)
            self.assertIn("pt-per-px", process.stderr)


if __name__ == "__main__":
    unittest.main()
