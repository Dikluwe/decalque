# Crystalline Lineage
# @prompt 00_nucleo/prompts/scan-sdf-font.md
# @layer L5
# @updated 2026-09-16

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from fontTools.pens.boundsPen import BoundsPen
from fontTools.ttLib import TTFont
from PIL import Image, ImageDraw, ImageFont


SCRIPT = pathlib.Path(__file__).parents[1] / "scan_font_builder.py"


class ScanSdfFontExecutionTests(unittest.TestCase):
    def write_manifest(self, root, samples, **overrides):
        manifest = {
            "schema_version": 1,
            "family": "Decalque SDF Test",
            "style": "Regular",
            "units_per_em": 1000,
            "metrics": {
                "ascender_px": 20,
                "descender_px": 5,
                "space_advance_px": 8,
            },
            "samples": samples,
        }
        manifest.update(overrides)
        path = root / "manifest.json"
        path.write_text(json.dumps(manifest), encoding="utf-8")
        return path

    def run_builder(self, root, manifest, mode="sdf-contour", output_name="derived.ttf"):
        output = root / output_name
        process = subprocess.run(
            [sys.executable, str(SCRIPT), str(manifest), str(output),
             "--vectorization", mode],
            capture_output=True,
            text=True,
            check=False,
        )
        return process, output

    @staticmethod
    def glyph_bounds(font_path, character):
        font = TTFont(font_path)
        try:
            glyph_name = font.getBestCmap()[ord(character)]
            glyph_set = font.getGlyphSet()
            pen = BoundsPen(glyph_set)
            glyph_set[glyph_name].draw(pen)
            return pen.bounds
        finally:
            font.close()

    def test_sdf_builds_renderable_ttf_preserving_o_counter_and_reports_geometry(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image = Image.new("L", (24, 26), 255)
            draw = ImageDraw.Draw(image)
            draw.ellipse((2, 2, 21, 22), fill=0)
            draw.ellipse((7, 7, 16, 17), fill=255)
            image.save(root / "O.png")
            manifest = self.write_manifest(root, [{
                "char": "O", "path": "O.png", "baseline_px": 23,
                "left_bearing_px": 2, "advance_px": 24,
            }])

            process, font_path = self.run_builder(root, manifest)

            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertTrue(font_path.is_file())
            report = json.loads(process.stdout)
            self.assertEqual(report["vectorization"], "sdf-contour")
            evidence = report["evidence"][0]
            self.assertEqual(evidence["sample_count"], 1)
            required_fields = {"aggregated_ink_pixels", "contour_count", "point_count"}
            self.assertTrue(
                required_fields.issubset(evidence),
                f"missing report fields: {sorted(required_fields - evidence.keys())}",
            )
            self.assertGreater(evidence["aggregated_ink_pixels"], 0)
            self.assertGreaterEqual(evidence["contour_count"], 2)
            self.assertGreater(evidence["point_count"], evidence["contour_count"])

            font = TTFont(font_path)
            try:
                glyph_name = font.getBestCmap()[ord("O")]
                self.assertGreaterEqual(font["glyf"][glyph_name].numberOfContours, 2)
            finally:
                font.close()
            rendered = Image.new("L", (140, 120), 255)
            raster_font = ImageFont.truetype(font_path, 80)
            ImageDraw.Draw(rendered).text((10, 5), "O", font=raster_font, fill=0)
            bbox = rendered.getbbox()
            self.assertIsNotNone(bbox)
            ink_bbox = Image.eval(rendered, lambda value: 255 - value).getbbox()
            self.assertIsNotNone(ink_bbox)
            left, top, right, bottom = ink_bbox
            center = rendered.getpixel(((left + right) // 2, (top + bottom) // 2))
            self.assertGreater(center, 220, "the O counter was filled")
            self.assertLess(rendered.getextrema()[0], 80)

    def test_sdf_median_rejects_one_geometric_outlier(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            samples = []
            for index, bounds in enumerate(((3, 4, 10, 19), (3, 4, 10, 19), (25, 4, 34, 19))):
                image = Image.new("L", (40, 24), 255)
                ImageDraw.Draw(image).rectangle(bounds, fill=0)
                image.save(root / f"I-{index}.png")
                samples.append({
                    "char": "I", "path": f"I-{index}.png", "baseline_px": 20,
                    "left_bearing_px": 3, "advance_px": 40,
                })
            manifest = self.write_manifest(root, samples)

            process, font_path = self.run_builder(root, manifest)

            self.assertEqual(process.returncode, 0, process.stderr)
            bounds = self.glyph_bounds(font_path, "I")
            self.assertIsNotNone(bounds)
            self.assertLess(bounds[2] - bounds[0], 600, bounds)
            report = json.loads(process.stdout)
            self.assertEqual(report["evidence"][0]["sample_count"], 3)

    def test_sdf_aligns_samples_by_baseline_and_left_bearing(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            first = Image.new("L", (20, 24), 255)
            ImageDraw.Draw(first).rectangle((2, 5, 7, 18), fill=0)
            first.save(root / "first.png")
            second = Image.new("L", (30, 30), 255)
            ImageDraw.Draw(second).rectangle((9, 10, 14, 23), fill=0)
            second.save(root / "second.png")
            manifest = self.write_manifest(root, [
                {"char": "H", "path": "first.png", "baseline_px": 19,
                 "left_bearing_px": 2, "advance_px": 20},
                {"char": "H", "path": "second.png", "baseline_px": 24,
                 "left_bearing_px": 9, "advance_px": 20},
            ])

            process, font_path = self.run_builder(root, manifest)

            self.assertEqual(process.returncode, 0, process.stderr)
            bounds = self.glyph_bounds(font_path, "H")
            self.assertIsNotNone(bounds)
            self.assertLess(bounds[2] - bounds[0], 400, bounds)
            self.assertLess(bounds[3] - bounds[1], 750, bounds)

    def test_explicit_scanline_mode_remains_renderable_and_preserves_metrics(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image = Image.new("L", (16, 24), 255)
            ImageDraw.Draw(image).rectangle((2, 3, 11, 18), fill=0)
            image.save(root / "R.png")
            manifest = self.write_manifest(root, [{
                "char": "R", "path": "R.png", "baseline_px": 19,
                "left_bearing_px": 2, "advance_px": 16,
            }])

            process, font_path = self.run_builder(root, manifest, mode="scanline")

            self.assertEqual(process.returncode, 0, process.stderr)
            report = json.loads(process.stdout)
            self.assertEqual(report["vectorization"], "scanline")
            font = TTFont(font_path)
            try:
                self.assertEqual(font.getBestCmap()[ord("R")], "uni0052")
                self.assertEqual(font["hmtx"]["uni0052"][0], 640)
            finally:
                font.close()
            rendered = Image.new("L", (100, 90), 255)
            ImageDraw.Draw(rendered).text(
                (5, 5), "R", font=ImageFont.truetype(font_path, 56), fill=0,
            )
            self.assertLess(rendered.getextrema()[0], 255)

    def test_sdf_rejects_visible_sample_without_a_zero_isoline(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            Image.new("L", (8, 8), 0).save(root / "solid.png")
            manifest = self.write_manifest(root, [{
                "char": "X", "path": "solid.png", "baseline_px": 8,
                "left_bearing_px": 0, "advance_px": 8,
            }], metrics={
                "ascender_px": 8, "descender_px": 2, "space_advance_px": 4,
            })

            process, font_path = self.run_builder(root, manifest)

            self.assertEqual(process.returncode, 2, process.stderr)
            self.assertFalse(font_path.exists())
            self.assertRegex(process.stderr.lower(), r"isoline|contour")


if __name__ == "__main__":
    unittest.main()
