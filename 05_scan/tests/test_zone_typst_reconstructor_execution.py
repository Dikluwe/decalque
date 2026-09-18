import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw


SCRIPT = pathlib.Path(__file__).parents[1] / "zone_typst_reconstructor.py"
FONT = pathlib.Path("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf")
CAN_RUN = FONT.exists() and all(shutil.which(item) for item in ("typst", "pdftoppm", "pdftotext"))


@unittest.skipUnless(CAN_RUN, "requires Typst, Poppler and DejaVu Serif")
class ZoneTypstReconstructorExecutionTests(unittest.TestCase):
    def test_materializes_frozen_lines_visual_and_searchable_pdf(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image = Image.new("RGB", (200, 120), "white")
            ImageDraw.Draw(image).rectangle((130, 20, 180, 70), fill="black")
            image_path = root / "page.png"
            image.save(image_path)
            zones = {"regions": [
                {"type": "paragraph",
                 "editorial_role": {"status": "observed", "role": "block_quote"},
                 "font_style_summary": {"status": "observed", "slant": "italic",
                                        "weight": "regular"},
                 "lines": [
                    {"text": "First physical line", "bbox": [10, 20, 115, 35]},
                    {"text": "Second physical line", "bbox": [10, 40, 125, 55]},
                ]},
                {"type": "visual", "bbox": [130, 20, 181, 71], "lines": []},
            ]}
            zones_path = root / "zones.json"
            zones_path.write_text(json.dumps(zones), encoding="utf-8")
            output = root / "result"

            process = subprocess.run([sys.executable, str(SCRIPT), str(zones_path), str(image_path),
                                      "--font", str(FONT), "--output-dir", str(output),
                                      "--width-pt", "120", "--height-pt", "72", "--dpi", "120"],
                                     capture_output=True, text=True, check=False)

            self.assertEqual(process.returncode, 0, process.stderr)
            for name in ("manifest.json", "page.typ", "page.pdf", "page.png", "comparison.png", "report.json"):
                self.assertTrue((output / name).is_file(), name)
            manifest = json.loads((output / "manifest.json").read_text())
            self.assertEqual([region["text"] for region in manifest["regions"] if region["kind"] == "text"],
                             ["First physical line", "Second physical line"])
            text_regions = [region for region in manifest["regions"] if region["kind"] == "text"]
            self.assertTrue(all(region["editorial_role"] == "block_quote"
                                for region in text_regions))
            self.assertTrue(all(region["font"]["style"] == "italic"
                                and region["font"]["skew_deg"] == -10
                                for region in text_regions))
            report = json.loads((output / "report.json").read_text())
            self.assertEqual(report["line_count"], 2)
            self.assertEqual(report["visual_count"], 1)


if __name__ == "__main__":
    unittest.main()
