# Crystalline Lineage
# @prompt _lab/scan_observer/specs/scan-glyph-segmentation.md
# @updated 2026-09-15

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw


SCRIPT = pathlib.Path(__file__).parents[1] / "scan_glyph_segmenter.py"
FONT_BUILDER = pathlib.Path(__file__).parents[1] / "scan_font_builder.py"


class ScanGlyphSegmenterExecutionTests(unittest.TestCase):
    def run_segmenter(self, root, image, text, baseline_px=20):
        image_path = root / "line.png"
        image.save(image_path)
        output_dir = root / "glyphs"
        process = subprocess.run(
            [sys.executable, str(SCRIPT), str(image_path), "--text", text,
             "--baseline-px", str(baseline_px), "--output-dir", str(output_dir)],
            capture_output=True, text=True, check=False,
        )
        manifest_path = output_dir / "manifest.json"
        manifest = json.loads(manifest_path.read_text()) if manifest_path.exists() else None
        return process, output_dir, manifest

    @staticmethod
    def two_separated_glyphs():
        image = Image.new("L", (70, 30), 255)
        draw = ImageDraw.Draw(image)
        draw.rectangle((5, 5, 19, 20), fill=0)
        draw.polygon([(42, 20), (51, 4), (60, 20)], fill=0)
        return image

    def test_repeated_characters_create_distinct_samples_without_overwrite(self):
        image = Image.new("L", (70, 30), 255)
        draw = ImageDraw.Draw(image)
        draw.polygon([(5, 20), (13, 4), (21, 20)], fill=0)
        draw.polygon([(42, 20), (50, 4), (58, 20)], fill=0)
        with tempfile.TemporaryDirectory() as directory:
            process, output_dir, manifest = self.run_segmenter(
                pathlib.Path(directory), image, "AA"
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertEqual([sample["char"] for sample in manifest["samples"]], ["A", "A"])
            paths = [sample["path"] for sample in manifest["samples"]]
            self.assertEqual(len(paths), 2)
            self.assertEqual(len(set(paths)), 2)
            for path in paths:
                glyph_path = pathlib.Path(path)
                if not glyph_path.is_absolute():
                    glyph_path = output_dir / glyph_path
                self.assertTrue(glyph_path.is_file(), glyph_path)
                self.assertLess(Image.open(glyph_path).convert("L").getextrema()[0], 255)

    def test_space_is_advance_without_png_and_manifest_builds_a_font(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            process, output_dir, manifest = self.run_segmenter(
                root, self.two_separated_glyphs(), "A B"
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertEqual(manifest["schema_version"], 1)
            self.assertEqual([sample["char"] for sample in manifest["samples"]], ["A", "B"])
            self.assertGreater(manifest["metrics"]["space_advance_px"], 0)
            self.assertFalse(any(sample["char"].isspace() for sample in manifest["samples"]))
            self.assertEqual(len(list(output_dir.rglob("*.png"))), 2)
            for sample in manifest["samples"]:
                self.assertGreater(sample["advance_px"], 0)
                self.assertIsInstance(sample["baseline_px"], int)
            font_path = root / "segmented.ttf"
            built = subprocess.run(
                [sys.executable, str(FONT_BUILDER), str(output_dir / "manifest.json"),
                 str(font_path)],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(built.returncode, 0, built.stderr)
            self.assertTrue(font_path.is_file())

    def test_local_baseline_preserves_descender_ink(self):
        image = Image.new("L", (72, 34), 255)
        draw = ImageDraw.Draw(image)
        draw.rectangle((5, 6, 18, 20), fill=0)
        draw.ellipse((42, 8, 58, 21), fill=0)
        draw.rectangle((54, 14, 58, 28), fill=0)
        with tempfile.TemporaryDirectory() as directory:
            process, output_dir, manifest = self.run_segmenter(
                pathlib.Path(directory), image, "Ag", baseline_px=21
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            samples = manifest["samples"]
            self.assertEqual([sample["char"] for sample in samples], ["A", "g"])
            descender = samples[1]
            glyph_path = pathlib.Path(descender["path"])
            if not glyph_path.is_absolute():
                glyph_path = output_dir / glyph_path
            glyph = Image.open(glyph_path).convert("L")
            local_baseline = descender["baseline_px"]
            self.assertGreater(local_baseline, 0)
            self.assertLess(local_baseline, glyph.height)
            self.assertLess(glyph.crop((0, local_baseline + 1, glyph.width, glyph.height)).getextrema()[0], 255)
            self.assertGreater(manifest["metrics"]["descender_px"], 0)

    def test_cut_through_connected_ink_is_reported_uncertain(self):
        image = Image.new("L", (64, 30), 255)
        draw = ImageDraw.Draw(image)
        draw.rectangle((5, 5, 58, 21), fill=0)
        with tempfile.TemporaryDirectory() as directory:
            process, _, manifest = self.run_segmenter(pathlib.Path(directory), image, "AB")
            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertEqual(len(manifest["samples"]), 2)
            for sample in manifest["samples"]:
                self.assertIn("bounds", sample)
                self.assertIn("confidence", sample)
            self.assertTrue(
                any(sample["confidence"] == "uncertain" for sample in manifest["samples"]),
                manifest["samples"],
            )

    def test_invalid_inputs_fail_without_manifest(self):
        cases = [
            (Image.new("L", (40, 24), 255), "A", 18),
            (self.two_separated_glyphs(), "   ", 20),
            (self.two_separated_glyphs(), "AB", 99),
        ]
        for image, text, baseline in cases:
            with self.subTest(text=repr(text), baseline=baseline):
                with tempfile.TemporaryDirectory() as directory:
                    process, output_dir, manifest = self.run_segmenter(
                        pathlib.Path(directory), image, text, baseline
                    )
                    self.assertNotEqual(process.returncode, 0)
                    self.assertIsNone(manifest)
                    self.assertFalse((output_dir / "manifest.json").exists())

        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            output_dir = root / "glyphs"
            process = subprocess.run(
                [sys.executable, str(SCRIPT), str(root / "missing.png"), "--text", "A",
                 "--baseline-px", "10", "--output-dir", str(output_dir)],
                capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(process.returncode, 0)
            self.assertFalse((output_dir / "manifest.json").exists())


if __name__ == "__main__":
    unittest.main()
