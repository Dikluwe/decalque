# Crystalline Lineage
# @prompt _lab/scan_observer/specs/line-diff-analysis.md
# @updated 2026-09-15

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw


SCRIPT = pathlib.Path(__file__).parents[1] / "line_diff_analyzer.py"


class LineDiffAnalyzerExecutionTests(unittest.TestCase):
    def run_case(self, reference, candidate, texts):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            reference.save(root / "reference.png"); candidate.save(root / "candidate.png")
            (root / "text.txt").write_text("\n".join(texts))
            process = subprocess.run([sys.executable, str(SCRIPT), str(root / "reference.png"),
                                      str(root / "candidate.png"), "--text-file", str(root / "text.txt")],
                                     capture_output=True, text=True, check=False)
            return process, json.loads(process.stdout) if process.returncode == 0 else None

    def test_reports_per_line_position_and_reflow_instead_of_global_average(self):
        reference = Image.new("L", (240, 140), 255); candidate = reference.copy()
        rd, cd = ImageDraw.Draw(reference), ImageDraw.Draw(candidate)
        rd.rectangle((20, 20, 180, 29), fill=0); rd.rectangle((20, 60, 160, 69), fill=0)
        cd.rectangle((26, 20, 186, 29), fill=0); cd.rectangle((20, 45, 100, 54), fill=0)
        cd.rectangle((20, 60, 160, 69), fill=0)
        process, output = self.run_case(reference, candidate, ["first line", "second line"])
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertGreaterEqual(output["summary"]["reflow"], 1)
        paired = [line for line in output["lines"] if line["alignment"] == "paired"]
        self.assertTrue(any(line["classification"] == "position" for line in paired))

    def test_localized_word_difference_enters_review_queue_only_with_observed_geometry(self):
        reference = Image.new("L", (260, 80), 255); candidate = reference.copy()
        rd, cd = ImageDraw.Draw(reference), ImageDraw.Draw(candidate)
        rd.rectangle((20, 20, 60, 35), fill=0); rd.rectangle((90, 20, 140, 35), fill=0)
        cd.rectangle((20, 20, 60, 35), fill=0); cd.ellipse((90, 20, 140, 35), fill=0)
        process, output = self.run_case(reference, candidate, ["alpha beta"])
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(output["lines"][0]["word_geometry_status"], "observed")
        self.assertGreaterEqual(output["summary"]["review_required"], 1)
        self.assertEqual(output["ocr_review_queue"][0]["status"], "review_required")

    def test_dimension_mismatch_is_unknown_process_failure(self):
        process, _ = self.run_case(Image.new("L", (20, 20), 255), Image.new("L", (21, 20), 255), [])
        self.assertEqual(process.returncode, 2)
        self.assertIn("dimensões", process.stderr)


if __name__ == "__main__": unittest.main()
