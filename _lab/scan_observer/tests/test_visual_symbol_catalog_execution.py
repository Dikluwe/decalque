import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw, ImageOps


SCRIPT = pathlib.Path(__file__).parents[1] / "visual_symbol_catalog.py"


class VisualSymbolCatalogExecutionTests(unittest.TestCase):
    def test_process_groups_mirrored_symbol_and_rejects_distinct_figure(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            leaf = Image.new("RGB", (80, 40), "white")
            ImageDraw.Draw(leaf).polygon([(8, 30), (50, 5), (70, 20), (45, 34)], fill="black")
            leaf.save(root / "leaf.png")
            ImageOps.mirror(leaf).save(root / "leaf-mirrored.png")
            figure = Image.new("RGB", (80, 40), "white")
            ImageDraw.Draw(figure).rectangle((10, 5, 70, 35), fill="black")
            figure.save(root / "figure.png")
            reports = []
            for index, name in enumerate(["leaf.png", "leaf-mirrored.png", "figure.png"]):
                report = root / f"page-{index}.json"
                report.write_text(json.dumps({"assets": [{"path": str(root / name)}]}))
                reports.append(report)
            process = subprocess.run(
                [sys.executable, str(SCRIPT), *map(str, reports)],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertEqual(len(output["symbols"]), 2)
            self.assertEqual(len(output["symbols"][0]["occurrences"]), 2)
            self.assertEqual(output["symbols"][0]["occurrences"][1]["orientation"], "mirrored")


if __name__ == "__main__":
    unittest.main()
