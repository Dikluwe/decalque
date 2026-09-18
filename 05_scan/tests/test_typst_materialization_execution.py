import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image


SCAN = pathlib.Path(__file__).parents[1]
MATERIALIZER = SCAN / "typst_page_materializer.py"
PIPELINE = SCAN / "typst_reconstruction_pipeline.py"


def manifest(image_name="mark.png"):
    return {
        "schema_version": 1,
        "page": {"width_pt": 200, "height_pt": 100, "source_width_px": 400, "source_height_px": 200},
        "regions": [
            {"kind": "image", "path": image_name, "bbox_px": [20, 20, 60, 60]},
            {"kind": "text", "text": 'Literal #[] \\ " text', "bbox_px": [100, 40, 380, 100],
             "font": {"family": "Liberation Serif", "size_pt": 12, "skew_deg": -12}, "align": "left"},
        ],
    }


class TypstMaterializationExecutionTests(unittest.TestCase):
    def test_real_process_generates_native_text_pdf_and_visual_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            Image.new("RGB", (20, 20), "black").save(root / "mark.png")
            manifest_path = root / "page.json"
            manifest_path.write_text(json.dumps(manifest()))
            source_typ = root / "source.typ"
            source_pdf = root / "source.pdf"
            source_typ.write_text('#set page(width: 200pt, height: 100pt, margin: 0pt)\nSource')
            compiled = subprocess.run(["typst", "compile", str(source_typ), str(source_pdf)], capture_output=True, text=True)
            self.assertEqual(compiled.returncode, 0, compiled.stderr)
            process = subprocess.run(
                [sys.executable, str(PIPELINE), str(manifest_path), str(source_pdf),
                 "--output-dir", str(root / "result"), "--dpi", "72"],
                capture_output=True, text=True, check=False,
            )
            self.assertEqual(process.returncode, 0, process.stderr)
            output = json.loads(process.stdout)
            self.assertIn("#skew(ax: -12deg)", pathlib.Path(output["typst"]).read_text())
            self.assertEqual(output["visual_comparison"]["status"], "comparable")
            self.assertTrue(pathlib.Path(output["pdf"]).exists())
            self.assertTrue(pathlib.Path(output["difference_png"]).exists())
            extracted = subprocess.run(["pdftotext", output["pdf"], "-"], capture_output=True, text=True)
            self.assertIn('Literal #[] \\ " text', extracted.stdout)

    def test_invalid_bbox_is_process_error_without_typst_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            data = manifest()
            data["regions"][0]["bbox_px"] = [10, 10, 10, 20]
            source = root / "page.json"
            output = root / "page.typ"
            source.write_text(json.dumps(data))
            process = subprocess.run([sys.executable, str(MATERIALIZER), str(source), str(output)],
                                     capture_output=True, text=True, check=False)
            self.assertEqual(process.returncode, 2)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
