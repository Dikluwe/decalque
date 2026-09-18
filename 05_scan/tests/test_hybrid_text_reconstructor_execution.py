# Crystalline Lineage
# @prompt 00_nucleo/prompts/hybrid-text-reconstruction.md
# @layer L5
# @updated 2026-09-16

import json
import math
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw, ImageFont


SCRIPT = pathlib.Path(__file__).parents[1] / "hybrid_text_reconstructor.py"
GUIDE_FONT = pathlib.Path("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf")


@unittest.skipUnless(GUIDE_FONT.is_file(), "DejaVuSerif is required for this execution contract")
class HybridTextReconstructorExecutionTests(unittest.TestCase):
    TEXT = "AVATAR"
    BASELINE_PX = 55
    INK = 73
    OBSTRUCTION = 37

    @classmethod
    def render_line(cls, *, obstruct=False):
        font = ImageFont.truetype(str(GUIDE_FONT), 42)
        x_offset = 10
        width = math.ceil(font.getlength(cls.TEXT)) + x_offset + 12
        image = Image.new("L", (width, 72), 255)
        ascent, _ = font.getmetrics()
        draw = ImageDraw.Draw(image)
        draw.text(
            (x_offset, cls.BASELINE_PX - ascent),
            cls.TEXT,
            font=font,
            fill=cls.INK,
        )
        if obstruct:
            prefix = font.getlength(cls.TEXT[:2])
            glyph_width = font.getlength(cls.TEXT[2])
            left = round(x_offset + prefix + glyph_width * 0.30)
            right = round(x_offset + prefix + glyph_width * 0.68)
            draw.rectangle((left, 13, right, cls.BASELINE_PX + 1), fill=cls.OBSTRUCTION)
        return image

    @staticmethod
    def resolve_artifact(output_dir, value):
        path = pathlib.Path(value)
        return path if path.is_absolute() else output_dir / path

    def run_reconstructor(
        self,
        root,
        image,
        *,
        guide_font=GUIDE_FONT,
        baseline_px=BASELINE_PX,
        ink_thinning=None,
    ):
        root.mkdir(parents=True, exist_ok=True)
        image_path = root / "line.png"
        image.save(image_path)
        output_dir = root / "hybrid"
        command = [
            sys.executable,
            str(SCRIPT),
            str(image_path),
            "--text",
            self.TEXT,
            "--baseline-px",
            str(baseline_px),
            "--font",
            str(guide_font),
            "--output-dir",
            str(output_dir),
            "--error-threshold",
            "0.45",
            "--coverage-threshold",
            "0.50",
        ]
        if ink_thinning is not None:
            command.extend(("--ink-thinning", str(ink_thinning)))
        process = subprocess.run(
            command,
            capture_output=True,
            text=True,
            check=False,
        )
        plan_path = output_dir / "plan.json"
        plan = json.loads(plan_path.read_text(encoding="utf-8")) if plan_path.exists() else None
        return process, output_dir, plan

    def test_compatible_line_is_mostly_native_and_writes_comparable_artifacts(self):
        self.assertTrue(SCRIPT.is_file(), SCRIPT)
        image = self.render_line(obstruct=True)
        with tempfile.TemporaryDirectory() as directory:
            process, output_dir, plan = self.run_reconstructor(
                pathlib.Path(directory), image
            )

            self.assertEqual(process.returncode, 0, process.stderr)
            self.assertIsNotNone(plan)
            occurrences = plan["occurrences"]
            self.assertEqual([item["char"] for item in occurrences], list(self.TEXT))
            decisions = [item["decision"] for item in occurrences]
            self.assertGreater(decisions.count("native"), decisions.count("raster"))
            self.assertIn("raster", decisions)
            self.assertEqual(decisions[2], "raster")

            intervals = plan["raster_intervals"]
            self.assertEqual(len(intervals), 1, intervals)
            self.assertEqual(intervals[0]["text"], self.TEXT[2])
            self.assertEqual(intervals[0]["start"], 2)
            self.assertEqual(intervals[0]["end"], 3)

            crop_path = self.resolve_artifact(output_dir, intervals[0]["path"])
            self.assertTrue(crop_path.is_file(), crop_path)
            crop_values = set(Image.open(crop_path).convert("L").getdata())
            self.assertIn(self.OBSTRUCTION, crop_values)
            self.assertTrue(crop_values.issubset(set(image.getdata())))

            artifacts = plan["artifacts"]
            for key in ("native", "hybrid", "residual"):
                artifact_path = self.resolve_artifact(output_dir, artifacts[key])
                self.assertTrue(artifact_path.is_file(), artifact_path)
                self.assertEqual(Image.open(artifact_path).size, image.size)

            metrics = plan["metrics"]
            numeric_metrics = {
                key: value
                for key, value in metrics.items()
                if isinstance(value, (int, float)) and not isinstance(value, bool)
            }
            self.assertTrue(numeric_metrics)
            self.assertTrue(all(math.isfinite(value) for value in numeric_metrics.values()))
            self.assertTrue(all(value >= 0 for value in numeric_metrics.values()))
            self.assertLessEqual(metrics["hybrid_error"], metrics["native_error"])

    def test_missing_font_white_line_and_invalid_baseline_fail_without_plan(self):
        self.assertTrue(SCRIPT.is_file(), SCRIPT)
        cases = (
            ("missing-font", self.render_line(), None, self.BASELINE_PX),
            ("white-line", Image.new("L", (220, 72), 255), GUIDE_FONT, self.BASELINE_PX),
            ("invalid-baseline", self.render_line(), GUIDE_FONT, 200),
        )
        for name, image, guide_font, baseline_px in cases:
            with self.subTest(case=name):
                with tempfile.TemporaryDirectory() as directory:
                    root = pathlib.Path(directory)
                    selected_font = guide_font or root / "missing-guide.ttf"
                    process, output_dir, plan = self.run_reconstructor(
                        root,
                        image,
                        guide_font=selected_font,
                        baseline_px=baseline_px,
                    )

                    self.assertNotEqual(process.returncode, 0)
                    self.assertIsNone(plan)
                    self.assertFalse((output_dir / "plan.json").exists())

    def test_ink_thinning_preserves_geometry_reduces_ink_and_rejects_invalid_values(self):
        self.assertTrue(SCRIPT.is_file(), SCRIPT)
        image = self.render_line()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            default_process, default_dir, default_plan = self.run_reconstructor(
                root / "default", image
            )
            zero_process, zero_dir, zero_plan = self.run_reconstructor(
                root / "zero", image, ink_thinning=0
            )
            thin_process, thin_dir, thin_plan = self.run_reconstructor(
                root / "thin", image, ink_thinning=0.35
            )

            for process in (default_process, zero_process, thin_process):
                self.assertEqual(process.returncode, 0, process.stderr)
            for plan in (default_plan, zero_plan, thin_plan):
                self.assertIsNotNone(plan)

            def native_image(output_dir, plan):
                path = self.resolve_artifact(output_dir, plan["artifacts"]["native"])
                with Image.open(path) as source:
                    return source.convert("L")

            default_native = native_image(default_dir, default_plan)
            zero_native = native_image(zero_dir, zero_plan)
            thin_native = native_image(thin_dir, thin_plan)

            self.assertEqual(zero_plan["ink_thinning"], 0)
            self.assertEqual(thin_plan["ink_thinning"], 0.35)
            self.assertEqual(list(zero_native.getdata()), list(default_native.getdata()))
            self.assertEqual(thin_native.size, default_native.size)
            self.assertLess(
                sum(pixel < 128 for pixel in thin_native.getdata()),
                sum(pixel < 128 for pixel in default_native.getdata()),
            )
            self.assertEqual(thin_plan["fit"], default_plan["fit"])
            self.assertEqual(
                [item["bounds"] for item in thin_plan["occurrences"]],
                [item["bounds"] for item in default_plan["occurrences"]],
            )

        for invalid_value in (-0.01, 1.01):
            with self.subTest(ink_thinning=invalid_value):
                with tempfile.TemporaryDirectory() as directory:
                    process, output_dir, plan = self.run_reconstructor(
                        pathlib.Path(directory),
                        image,
                        ink_thinning=invalid_value,
                    )
                    self.assertNotEqual(process.returncode, 0)
                    self.assertIsNone(plan)
                    self.assertFalse((output_dir / "plan.json").exists())


if __name__ == "__main__":
    unittest.main()
