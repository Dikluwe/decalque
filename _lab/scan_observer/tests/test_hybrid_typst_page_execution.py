import json
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw


SCAN = pathlib.Path(__file__).parents[1]
MATERIALIZER = SCAN / "hybrid_typst_page.py"
DEJAVU_SERIF = pathlib.Path("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf")
REQUIRED_TOOLS = ("typst", "pdftoppm", "pdftotext")
CAN_EXECUTE = DEJAVU_SERIF.exists() and all(
    shutil.which(tool) for tool in REQUIRED_TOOLS
)


def hybrid_manifest(font_file="DejaVuSerif.ttf"):
    return {
        "schema_version": 1,
        "page": {
            "image": "scan.png",
            "width_px": 120,
            "height_px": 80,
            "source_width_px": 120,
            "source_height_px": 80,
            "width_pt": 72,
            "height_pt": 48,
            "dpi": 120,
        },
        "lines": [
            {
                "transcription": "Alpha raster Omega",
                "bbox_px": [10, 20, 110, 46],
                "baseline_px": 41,
                "font": {
                    "family": "DejaVu Serif",
                    "file": font_file,
                    "size_pt": 10,
                },
                "scale_x": 1.0,
                "tracking_pt": 0.0,
                "segments": [
                    {"kind": "text", "text": "Alpha ", "bbox_px": [10, 20, 42, 46]},
                    {
                        "kind": "raster",
                        "text": "raster",
                        "source_bbox_px": [42, 20, 78, 46],
                        "bbox_px": [42, 20, 78, 46],
                    },
                    {"kind": "text", "text": " Omega", "bbox_px": [78, 20, 110, 46]},
                ],
                "plan": "line-plan.json",
            }
        ],
    }


def write_fixture(root, data=None, install_font=True):
    data = data or hybrid_manifest()
    image = Image.new("RGB", (120, 80), "white")
    draw = ImageDraw.Draw(image)
    draw.rectangle((42, 20, 77, 45), fill="black")
    image.save(root / "scan.png")
    fonts = root / "fonts"
    fonts.mkdir()
    if install_font:
        shutil.copy2(DEJAVU_SERIF, fonts / "DejaVuSerif.ttf")
    line = data["lines"][0]
    plan = {
        key: line[key]
        for key in ("transcription", "bbox_px", "baseline_px", "font", "scale_x", "tracking_pt", "segments")
    }
    root.joinpath("line-plan.json").write_text(json.dumps(plan), encoding="utf-8")
    manifest_path = root / "page.json"
    manifest_path.write_text(json.dumps(data), encoding="utf-8")
    return manifest_path, fonts


def run_materializer(manifest_path, output_dir, font_path):
    return subprocess.run(
        [
            sys.executable,
            str(MATERIALIZER),
            str(manifest_path),
            "--output-dir",
            str(output_dir),
            "--font",
            str(font_path),
        ],
        capture_output=True,
        text=True,
        check=False,
    )


@unittest.skipUnless(
    CAN_EXECUTE,
    "requires DejaVuSerif, typst, pdftoppm, and pdftotext",
)
class HybridTypstPageExecutionTests(unittest.TestCase):
    def test_vector_debug_mode_exposes_text_boxes_and_baselines(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            data = hybrid_manifest()
            data["visible_mode"] = "vector-debug"
            manifest_path, fonts = write_fixture(root, data)
            output = root / "result"

            process = run_materializer(manifest_path, output, fonts / "DejaVuSerif.ttf")

            self.assertEqual(process.returncode, 0, process.stderr)
            typst = output.joinpath("page.typ").read_text(encoding="utf-8")
            self.assertIn("fill: rgb(255, 255, 255, 72%)", typst)
            self.assertIn("paint: blue", typst)
            self.assertIn("paint: green", typst)
            self.assertIn("fill: red", typst)
            report = json.loads(output.joinpath("report.json").read_text(encoding="utf-8"))
            self.assertEqual(
                report["visible_text_mode"],
                "vector-debug-overlay-with-transparent-search-layer",
            )

    def test_real_cli_preserves_page_geometry_raster_and_complete_searchable_text(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            manifest_path, fonts = write_fixture(root)
            output = root / "result"

            process = run_materializer(manifest_path, output, fonts / "DejaVuSerif.ttf")

            self.assertEqual(process.returncode, 0, process.stderr)
            expected_artifacts = {
                "typst": output / "page.typ",
                "pdf": output / "page.pdf",
                "raster": output / "page.png",
                "report": output / "report.json",
            }
            for label, path in expected_artifacts.items():
                self.assertTrue(path.is_file(), f"missing {label} artifact: {path}")
                self.assertGreater(path.stat().st_size, 0, f"empty {label} artifact: {path}")

            rendered_prefix = root / "independent-render"
            rendered = subprocess.run(
                [
                    "pdftoppm",
                    "-f",
                    "1",
                    "-singlefile",
                    "-r",
                    "120",
                    "-png",
                    str(expected_artifacts["pdf"]),
                    str(rendered_prefix),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(rendered.returncode, 0, rendered.stderr)
            with Image.open(rendered_prefix.with_suffix(".png")) as image:
                self.assertEqual(image.size, (120, 80))
            with Image.open(expected_artifacts["raster"]) as image:
                self.assertEqual(image.size, (120, 80))

            extracted = subprocess.run(
                ["pdftotext", str(expected_artifacts["pdf"]), "-"],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(extracted.returncode, 0, extracted.stderr)
            normalized_text = re.sub(r"\s+", " ", extracted.stdout).strip()
            self.assertIn("Alpha raster Omega", normalized_text)

            report = json.loads(expected_artifacts["report"].read_text(encoding="utf-8"))
            self.assertEqual(report.get("status"), "success")

    def test_invalid_plans_and_missing_fonts_fail_without_success_report(self):
        invalid_cases = []

        degenerate = hybrid_manifest()
        degenerate["lines"][0]["bbox_px"] = [10, 20, 10, 46]
        invalid_cases.append(("degenerate-line-box", degenerate, True))

        inconsistent = hybrid_manifest()
        inconsistent["lines"][0]["segments"][1]["text"] = "wrong"
        invalid_cases.append(("inconsistent-raster-plan", inconsistent, True))

        invalid_cases.append(("missing-font", hybrid_manifest("AbsentSerif.ttf"), False))

        for name, data, install_font in invalid_cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                manifest_path, fonts = write_fixture(root, data, install_font=install_font)
                output = root / "result"

                font_path = fonts / data["lines"][0]["font"]["file"]
                process = run_materializer(manifest_path, output, font_path)

                self.assertNotEqual(process.returncode, 0, process.stdout)
                self.assertFalse(output.joinpath("report.json").exists())


if __name__ == "__main__":
    unittest.main()
