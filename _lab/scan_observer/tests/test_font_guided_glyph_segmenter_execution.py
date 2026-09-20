# Crystalline Lineage
# @prompt _lab/scan_observer/specs/scan-glyph-segmentation.md
# @updated 2026-09-16

import json
import math
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw, ImageFont


SCRIPT = pathlib.Path(__file__).parents[1] / "scan_glyph_segmenter.py"
GUIDE_FONT = pathlib.Path("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf")


@unittest.skipUnless(GUIDE_FONT.is_file(), "DejaVuSerif is required for this execution contract")
class FontGuidedGlyphSegmenterExecutionTests(unittest.TestCase):
    def run_segmenter(self, root, image, text, guide_font=GUIDE_FONT, baseline_px=52):
        image_path = root / "line.png"
        image.save(image_path)
        output_dir = root / "glyphs"
        process = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                str(image_path),
                "--text",
                text,
                "--baseline-px",
                str(baseline_px),
                "--output-dir",
                str(output_dir),
                "--guide-font",
                str(guide_font),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        manifest_path = output_dir / "manifest.json"
        manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else None
        return process, output_dir, manifest

    @staticmethod
    def render_line(text, font_size=42, ink=73, baseline_px=52):
        font = ImageFont.truetype(str(GUIDE_FONT), font_size)
        width = math.ceil(font.getlength(text)) + 20
        image = Image.new("L", (width, 68), 255)
        ascent, _ = font.getmetrics()
        ImageDraw.Draw(image).text((10, baseline_px - ascent), text, font=font, fill=ink)
        return image

    @staticmethod
    def horizontal_bounds(sample):
        bounds = sample["bounds"]
        if isinstance(bounds, dict):
            left = bounds.get("left_px", bounds.get("left", bounds.get("x0")))
            right = bounds.get("right_px", bounds.get("right", bounds.get("x1")))
        else:
            left, right = bounds[0], bounds[2] if len(bounds) == 4 else bounds[1]
        return left, right

    @staticmethod
    def sample_path(output_dir, sample):
        path = pathlib.Path(sample["path"])
        return path if path.is_absolute() else output_dir / path

    def test_font_guided_execution_records_fit_and_ordered_valid_bounds(self):
        text = "AV AAV"
        image = self.render_line(text)
        with tempfile.TemporaryDirectory() as directory:
            process, output_dir, manifest = self.run_segmenter(
                pathlib.Path(directory), image, text
            )

            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertIsNotNone(manifest)
            segmentation = manifest["segmentation"]
            method = segmentation["method"]
            self.assertIsInstance(method, str)
            self.assertIn("font", method.lower())
            self.assertIn("guid", method.lower())
            parameters = segmentation.get("parameters", segmentation.get("guide"))
            self.assertIsInstance(parameters, dict)
            fitted_values = [
                value
                for value in parameters.values()
                if isinstance(value, (int, float)) and not isinstance(value, bool)
            ]
            self.assertTrue(fitted_values)
            self.assertTrue(all(math.isfinite(value) for value in fitted_values))
            self.assertIsInstance(segmentation["fit_error"], (int, float))
            self.assertTrue(math.isfinite(segmentation["fit_error"]))
            self.assertGreaterEqual(segmentation["fit_error"], 0)

            samples = manifest["samples"]
            self.assertEqual([sample["char"] for sample in samples], list("AVAAV"))
            previous_left = -1
            previous_right = -1
            for sample in samples:
                left, right = self.horizontal_bounds(sample)
                self.assertIsInstance(left, int)
                self.assertIsInstance(right, int)
                self.assertGreaterEqual(left, 0)
                self.assertGreater(right, left)
                self.assertLessEqual(right, image.width)
                self.assertGreater(left, previous_left)
                self.assertGreater(right, previous_right)
                previous_left = left
                previous_right = right
                self.assertTrue(self.sample_path(output_dir, sample).is_file())
            self.assertGreater(manifest["metrics"]["space_advance_px"], 0)

    def test_samples_preserve_distinctive_input_intensity_instead_of_guide_pixels(self):
        text = "AV AAV"
        image = self.render_line(text, ink=73)
        with tempfile.TemporaryDirectory() as directory:
            process, output_dir, manifest = self.run_segmenter(
                pathlib.Path(directory), image, text
            )

            self.assertEqual(process.returncode, 0, process.stderr)
            sample_values = set()
            for sample in manifest["samples"]:
                glyph = Image.open(self.sample_path(output_dir, sample)).convert("L")
                sample_values.update(glyph.getdata())
            self.assertIn(73, sample_values)
            self.assertTrue(sample_values.issubset(set(image.getdata())))

    def test_missing_guide_font_fails_without_manifest(self):
        image = self.render_line("AV")
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            process, output_dir, manifest = self.run_segmenter(
                root, image, "AV", guide_font=root / "missing-guide.ttf"
            )

            self.assertNotEqual(process.returncode, 0)
            self.assertIsNone(manifest)
            self.assertFalse((output_dir / "manifest.json").exists())

    def test_poor_font_fit_marks_at_least_one_sample_uncertain(self):
        image = Image.new("L", (190, 68), 255)
        draw = ImageDraw.Draw(image)
        # Equal-width blocks at deliberately irregular advances cannot match the
        # alternating wide/narrow advances of DejaVuSerif "WiWi" with one fit.
        for left in (5, 31, 104, 166):
            draw.rectangle((left, 13, left + 14, 52), fill=73)

        with tempfile.TemporaryDirectory() as directory:
            process, _, manifest = self.run_segmenter(
                pathlib.Path(directory), image, "WiWi"
            )

            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertTrue(
                any(sample["confidence"] == "uncertain" for sample in manifest["samples"]),
                manifest["samples"],
            )


if __name__ == "__main__":
    unittest.main()
