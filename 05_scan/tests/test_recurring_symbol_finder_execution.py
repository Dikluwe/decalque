import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw, ImageOps


SCRIPT = pathlib.Path(__file__).parents[1] / "recurring_symbol_finder.py"


class RecurringSymbolFinderExecutionTests(unittest.TestCase):
    def test_process_finds_scaled_mirrored_symbol_in_page(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            template = Image.new("RGB", (80, 40), "white")
            ImageDraw.Draw(template).polygon([(8, 30), (50, 5), (70, 20), (45, 34)], fill="black")
            template.save(root / "template.png")
            page = Image.new("RGB", (300, 200), "white")
            mirrored = ImageOps.mirror(template).resize((40, 20), Image.Resampling.NEAREST)
            page.paste(mirrored, (130, 60))
            page.save(root / "page.png")
            process = subprocess.run(
                [sys.executable, str(SCRIPT), str(root / "template.png"), str(root / "page.png"),
                 "--asset-dir", str(root / "assets")],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertEqual(len(output["detections"]), 1)
            self.assertEqual(output["detections"][0]["orientation"], "mirrored")
            self.assertGreaterEqual(output["detections"][0]["similarity"], 0.65)
            self.assertTrue(pathlib.Path(output["detections"][0]["path"]).exists())


if __name__ == "__main__":
    unittest.main()
