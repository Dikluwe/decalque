# Crystalline Lineage
# @prompt 00_nucleo/prompts/scan-derived-font.md
# @layer L5
# @updated 2026-09-15

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw, ImageFont
from fontTools.ttLib import TTFont


SCRIPT = pathlib.Path(__file__).parents[1] / "scan_font_builder.py"


class ScanFontBuilderExecutionTests(unittest.TestCase):
    def test_builds_loadable_font_with_observed_unicode_metrics_and_multiple_samples(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            for index, offset in enumerate((0, 1)):
                image = Image.new("L", (18, 24), 255); draw = ImageDraw.Draw(image)
                draw.polygon([(2 + offset, 18), (8, 3), (14, 18)], fill=0)
                draw.rectangle((5, 11, 11, 13), fill=255); image.save(root / f"A-{index}.png")
            image = Image.new("L", (16, 24), 255); draw = ImageDraw.Draw(image)
            draw.rectangle((2, 3, 5, 18), fill=0); draw.ellipse((3, 3, 14, 11), fill=0)
            draw.ellipse((3, 10, 14, 18), fill=0); image.save(root / "B.png")
            manifest = {"schema_version": 1, "family": "Decalque Test", "style": "Regular",
                        "units_per_em": 1000,
                        "metrics": {"ascender_px": 20, "descender_px": 5, "space_advance_px": 8},
                        "samples": [
                            {"char": "A", "path": "A-0.png", "baseline_px": 19, "advance_px": 18},
                            {"char": "A", "path": "A-1.png", "baseline_px": 19, "advance_px": 18},
                            {"char": "B", "path": "B.png", "baseline_px": 19, "advance_px": 16}]}
            (root / "manifest.json").write_text(json.dumps(manifest)); font_path = root / "derived.ttf"
            process = subprocess.run([sys.executable, str(SCRIPT), str(root / "manifest.json"), str(font_path)],
                                     capture_output=True, text=True, check=False)
            self.assertEqual(process.returncode, 0, process.stderr)
            report = json.loads(process.stdout); self.assertEqual(report["coverage"], ["A", "B"])
            self.assertEqual(report["evidence"][0]["sample_count"], 2)
            font = TTFont(font_path)
            self.assertEqual(font.getBestCmap()[ord("A")], "uni0041")
            self.assertEqual(font["hmtx"]["uni0041"][0], 720)
            font.close()
            rendered = Image.new("L", (200, 100), 255)
            ImageDraw.Draw(rendered).text((5, 5), "AB BA", font=ImageFont.truetype(font_path, 64), fill=0)
            self.assertLess(rendered.getextrema()[0], 255)

    def test_rejects_ambiguous_multicharacter_label(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory); Image.new("L", (10, 10), 255).save(root / "x.png")
            data = {"schema_version": 1, "family": "Bad", "metrics": {"ascender_px": 8, "descender_px": 2},
                    "samples": [{"char": "rn", "path": "x.png", "baseline_px": 8, "advance_px": 10}]}
            (root / "manifest.json").write_text(json.dumps(data))
            process = subprocess.run([sys.executable, str(SCRIPT), str(root / "manifest.json"), str(root / "bad.ttf")],
                                     capture_output=True, text=True, check=False)
            self.assertEqual(process.returncode, 2); self.assertFalse((root / "bad.ttf").exists())


if __name__ == "__main__": unittest.main()
