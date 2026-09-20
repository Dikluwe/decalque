import importlib.util
import json
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest

from PIL import Image


ROOT = pathlib.Path(__file__).parents[1]
SCRIPT = ROOT / "editorial_typst_composer.py"
SPEC = importlib.util.spec_from_file_location("editorial_typst_composer", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)
FONT = pathlib.Path("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf")
CAN_RENDER = FONT.exists() and all(shutil.which(x) for x in ("typst", "pdftotext", "pdftoppm"))


def classified_page():
    return {
        "schema_version": 1,
        "page": {"width_px": 200, "height_px": 300},
        "regions": [
            {"id": "R1", "type": "heading", "bbox": [20, 20, 180, 50],
             "editorial_role": {"status": "observed", "role": "heading"},
             "font_style_summary": {"weight": "bold", "slant": "normal"},
             "lines": [{"text": "A heading", "bbox": [20, 20, 120, 40]}]},
            {"id": "R2", "type": "visual", "bbox": [20, 70, 100, 150],
             "editorial_role": {"status": "observed", "role": "visual"}, "lines": []},
            {"id": "R3", "type": "list", "bbox": [20, 155, 100, 180],
             "editorial_role": {"status": "observed", "role": "figure_caption"},
             "font_style_summary": {"weight": "regular", "slant": "italic"},
             "lines": [{"text": "Figure 1. Device", "bbox": [20, 155, 100, 175]}]},
            {"id": "R4", "type": "paragraph", "bbox": [20, 200, 180, 250],
             "editorial_role": {"status": "unknown", "role": "unknown"},
             "lines": [{"text": "Uncertain reading", "bbox": [20, 200, 150, 220]}]},
        ],
    }


class EditorialManifestContractTests(unittest.TestCase):
    def test_keeps_roles_lines_spacing_and_reserved_visual(self):
        manifest = MODULE.build_manifest(classified_page(), None, 120, 180, "DejaVu Serif")
        self.assertEqual(manifest["page"]["side"], "right")
        self.assertEqual([b["role"] for b in manifest["blocks"]],
                         ["heading", "visual", "figure_caption", "unknown"])
        self.assertEqual(manifest["blocks"][1]["strategy"], "reserved_visual")
        self.assertEqual(manifest["blocks"][2]["related_visual_id"], "R2")
        self.assertEqual(manifest["blocks"][3]["strategy"], "unknown")
        self.assertEqual(manifest["blocks"][2]["space_before_px"], 5)
        self.assertEqual(manifest["blocks"][0]["lines"][0]["text"], "A heading")

    def test_rejects_unobserved_role_that_claims_a_known_class(self):
        page = classified_page()
        page["regions"][0]["editorial_role"]["status"] = "inferred"
        with self.assertRaisesRegex(ValueError, "papel editorial não observado"):
            MODULE.build_manifest(page, None, 120, 180, "DejaVu Serif")


@unittest.skipUnless(CAN_RENDER, "requires Typst, Poppler and DejaVu Serif")
class EditorialComposerExecutionTests(unittest.TestCase):
    def test_creates_common_library_searchable_pdf_and_reserved_visual(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "source.png"
            Image.new("RGB", (200, 300), "white").save(source)
            classified = root / "classified.json"
            classified.write_text(json.dumps(classified_page()), encoding="utf-8")
            output = root / "result"
            process = subprocess.run([
                sys.executable, str(SCRIPT), str(classified), str(source),
                "--output-dir", str(output), "--font", str(FONT),
                "--width-pt", "120", "--height-pt", "180", "--page-number", "1",
            ], capture_output=True, text=True, check=False)
            self.assertEqual(process.returncode, 0, process.stderr)
            for name in ("manifest.json", "editorial-blocks.typ", "page.typ", "page.pdf",
                         "page.png", "comparison.png", "report.json"):
                self.assertTrue((output / name).is_file(), name)
            typ = (output / "page.typ").read_text(encoding="utf-8")
            self.assertIn('#import "editorial-blocks.typ": *', typ)
            self.assertIn('editorial-line(role: "heading"', typ)
            self.assertIn('content: text("A heading")', typ)
            self.assertNotIn("scale(x:", (output / "editorial-blocks.typ").read_text())
            self.assertIn('reserved-visual(label: "visual R2"', typ)
            extracted = subprocess.run(["pdftotext", str(output / "page.pdf"), "-"],
                                       capture_output=True, text=True, check=True).stdout
            self.assertIn("A heading", extracted)
            self.assertIn("Uncertain reading", extracted)
            report = json.loads((output / "report.json").read_text())
            self.assertEqual(set(report["timings_seconds"]), {
                "load_and_manifest", "crop_visuals", "write_sources", "typst_compile",
                "pdf_rasterize", "raster_diff", "text_verify", "total",
            })
            self.assertTrue(all(value >= 0 for value in report["timings_seconds"].values()))


if __name__ == "__main__":
    unittest.main()
