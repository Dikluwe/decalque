# Crystalline Lineage
# @prompt _lab/scan_observer/specs/joint-glyph-reconstruction.md
# @updated 2026-09-16

import json
import math
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw, ImageFont


SCRIPT = pathlib.Path(__file__).parents[1] / "joint_glyph_reconstructor.py"
FONT_BUILDER = pathlib.Path(__file__).parents[1] / "scan_font_builder.py"
GUIDE_FONT = pathlib.Path("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf")


@unittest.skipUnless(GUIDE_FONT.is_file(), "DejaVuSerif is required for this execution contract")
class JointGlyphReconstructorExecutionTests(unittest.TestCase):
    @staticmethod
    def render_line(path, text, *, ink=73, baseline_px=54, x_offset=10):
        font = ImageFont.truetype(str(GUIDE_FONT), 42)
        width = math.ceil(font.getlength(text)) + x_offset + 12
        image = Image.new("L", (width, 70), 255)
        ascent, _ = font.getmetrics()
        ImageDraw.Draw(image).text(
            (x_offset, baseline_px - ascent), text, font=font, fill=ink
        )
        image.save(path)
        return image

    def write_corpus(self, root, *, include_validation=True, white_line=False):
        specifications = [
            ("train-a", "AVA", "train", 10),
            ("train-b", "VAV", "train", 12),
            ("validation-a", "AVA", "validation", 9),
        ]
        lines = []
        for line_id, text, split, offset in specifications:
            if split == "validation" and not include_validation:
                continue
            image_path = root / f"{line_id}.png"
            if white_line and line_id == "train-b":
                Image.new("L", (150, 70), 255).save(image_path)
            else:
                self.render_line(image_path, text, x_offset=offset)
            lines.append(
                {
                    "id": line_id,
                    "image": image_path.name,
                    "text": text,
                    "baseline_px": 54,
                    "split": split,
                }
            )
        corpus_path = root / "corpus.json"
        corpus_path.write_text(
            json.dumps({"schema_version": 1, "lines": lines}), encoding="utf-8"
        )
        return corpus_path, lines

    @staticmethod
    def run_reconstructor(root, corpus_path, guide_font=GUIDE_FONT):
        output_dir = root / "reconstruction"
        process = subprocess.run(
            [
                sys.executable,
                str(SCRIPT),
                str(corpus_path),
                "--guide-font",
                str(guide_font),
                "--output-dir",
                str(output_dir),
                "--iterations",
                "4",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        return process, output_dir

    @staticmethod
    def resolve_artifact(output_dir, path):
        artifact = pathlib.Path(path)
        return artifact if artifact.is_absolute() else output_dir / artifact

    def test_joint_execution_writes_shared_atlas_reconstructions_and_finite_history(self):
        self.assertTrue(SCRIPT.is_file(), SCRIPT)
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            corpus_path, lines = self.write_corpus(root)

            process, output_dir = self.run_reconstructor(root, corpus_path)

            self.assertEqual(process.returncode, 0, process.stderr)
            report_path = output_dir / "report.json"
            manifest_path = output_dir / "manifest.json"
            self.assertTrue(report_path.is_file())
            self.assertTrue(manifest_path.is_file())
            report = json.loads(report_path.read_text(encoding="utf-8"))
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

            history = report["history"]
            self.assertGreaterEqual(len(history), 1)
            self.assertLessEqual(len(history), 4)
            for iteration in history:
                for key in ("train_error", "validation_error"):
                    self.assertIsInstance(iteration[key], (int, float))
                    self.assertTrue(math.isfinite(iteration[key]))
                    self.assertGreaterEqual(iteration[key], 0)
            best_iteration = report["best_iteration"]
            self.assertIsInstance(best_iteration, int)
            matching_iterations = [
                item for item in history if item["iteration"] == best_iteration
            ]
            self.assertEqual(len(matching_iterations), 1)
            best_validation = matching_iterations[0]["validation_error"]
            self.assertAlmostEqual(
                best_validation,
                min(item["validation_error"] for item in history),
            )
            baseline_validation = history[0]["validation_error"]
            self.assertLessEqual(best_validation, baseline_validation)
            self.assertEqual(report["best"]["iteration"], best_iteration)
            self.assertAlmostEqual(report["best"]["validation_error"], best_validation)

            self.assertEqual(manifest["schema_version"], 1)
            self.assertEqual(
                {sample["char"] for sample in manifest["samples"]}, {"A", "V"}
            )
            self.assertEqual(len(manifest["samples"]), 2)
            self.assertEqual(report["occurrences"]["A"], 3)
            self.assertEqual(report["occurrences"]["V"], 3)

            glyph_values = set()
            glyph_paths = []
            for sample in manifest["samples"]:
                self.assertGreater(sample["advance_px"], 0)
                self.assertIsInstance(sample["baseline_px"], int)
                self.assertEqual(sample["occurrence_count"], 3)
                glyph_path = self.resolve_artifact(output_dir, sample["path"])
                glyph_paths.append(glyph_path)
                self.assertTrue(glyph_path.is_file(), glyph_path)
                glyph_values.update(Image.open(glyph_path).convert("L").getdata())
            self.assertEqual(len(glyph_paths), len(set(glyph_paths)))
            self.assertIn(73, glyph_values)

            reconstructions = sorted((output_dir / "reconstructions").glob("*.png"))
            self.assertEqual(len(reconstructions), len(lines))
            for image_path in reconstructions:
                self.assertTrue(image_path.is_file(), image_path)
                self.assertEqual(image_path.suffix.lower(), ".png")

            font_path = root / "joint-derived.ttf"
            built = subprocess.run(
                [sys.executable, str(FONT_BUILDER), str(manifest_path), str(font_path)],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(built.returncode, 0, built.stderr)
            self.assertTrue(font_path.is_file())

    def test_invalid_corpora_and_missing_font_fail_without_success_report(self):
        self.assertTrue(SCRIPT.is_file(), SCRIPT)
        cases = (
            ("no-validation", {"include_validation": False}, GUIDE_FONT),
            ("white-line", {"white_line": True}, GUIDE_FONT),
            ("missing-font", {}, None),
        )
        for name, corpus_options, guide_font in cases:
            with self.subTest(case=name):
                with tempfile.TemporaryDirectory() as directory:
                    root = pathlib.Path(directory)
                    corpus_path, _ = self.write_corpus(root, **corpus_options)
                    selected_font = guide_font or root / "missing-guide.ttf"

                    process, output_dir = self.run_reconstructor(
                        root, corpus_path, selected_font
                    )

                    self.assertNotEqual(process.returncode, 0)
                    self.assertFalse((output_dir / "report.json").exists())


if __name__ == "__main__":
    unittest.main()
