import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT))
SPEC = importlib.util.spec_from_file_location(
    "editorial_book_composer", ROOT / "editorial_book_composer.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class EditorialBookComposerTests(unittest.TestCase):
    def test_discovers_intersection_and_reports_missing_counterparts(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            classified, images = root / "classified", root / "images"
            classified.mkdir(); images.mkdir()
            for number in (1, 2, 3):
                (classified / f"page-{number:03d}.json").write_text("{}")
            for number in (1, 3, 4):
                (images / f"page-{number:03d}.png").write_bytes(b"png")
            discovered = MODULE.discover_pages(classified, images, 1, 4)
            self.assertEqual([item[0] for item in discovered["ready"]], [1, 3])
            self.assertEqual(discovered["missing_images"], [2])
            self.assertEqual(discovered["missing_classified"], [4])

    def test_resume_skips_only_successful_page_and_aggregates_timings(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            classified, images, output = root / "classified", root / "images", root / "out"
            classified.mkdir(); images.mkdir(); output.mkdir()
            pages = []
            for number in (1, 2):
                source_json = classified / f"page-{number:03d}.json"
                source_png = images / f"page-{number:03d}.png"
                source_json.write_text("{}")
                source_png.write_bytes(b"png")
                pages.append((number, source_json, source_png))
            done = output / "pages" / "page-001"
            done.mkdir(parents=True)
            (done / "page.pdf").write_bytes(b"pdf")
            (done / "report.json").write_text(json.dumps({"status": "success"}))

            def fake_compose(*args, **kwargs):
                target = args[2]
                target.mkdir(parents=True, exist_ok=True)
                (target / "page.pdf").write_bytes(b"pdf")
                return {"status": "success", "timings_seconds": {
                    "typst_compile": 0.4, "pdf_rasterize": 0.2, "total": 0.8}}

            with mock.patch.object(MODULE.editorial_typst_composer, "compose", fake_compose):
                result = MODULE.process_pages(pages, output, pathlib.Path("font.ttf"),
                                              452.16, 714.24, 150, 1, True)
            self.assertEqual(result["processed_pages"], [2])
            self.assertEqual(result["skipped_pages"], [1])
            self.assertEqual(result["partial_pages"], [])
            self.assertEqual(result["failed_pages"], [])
            self.assertEqual(result["timings_seconds"]["typst_compile"], 0.4)


if __name__ == "__main__":
    unittest.main()
