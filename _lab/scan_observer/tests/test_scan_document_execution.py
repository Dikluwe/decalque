import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


ROOT = pathlib.Path(__file__).parents[3]
SCAN = ROOT / "_lab/scan_observer"
FIXTURES = SCAN / "tests" / "fixtures"


class ScanDocumentExecutionTests(unittest.TestCase):
    def compile_document(self, directory):
        pdf = pathlib.Path(directory) / "book.pdf"
        result = subprocess.run(
            ["typst", "compile", str(FIXTURES / "document-three-pages.typ"), str(pdf)],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return pdf

    def run_document(self, pdf, *arguments):
        return subprocess.run(
            [
                sys.executable, str(SCAN / "scan_compare_document.py"), str(pdf), str(pdf),
                "--pipeline", str(FIXTURES / "fake_scan_compare_pipeline.py"),
                "--python", sys.executable, *arguments,
            ],
            cwd=ROOT, capture_output=True, text=True, check=False,
        )

    def test_rasterizes_and_compares_pages_sequentially(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_document(self.compile_document(directory), "--dpi", "96")
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(report["page_count"], 3)
        self.assertEqual(report["dpi"], 96)
        self.assertEqual(report["verdict"], {
            "status": "preserved",
            "page_statuses": ["preserved", "preserved", "preserved"],
        })
        self.assertEqual(
            [page["page_index"] for page in report["pages"]], [0, 1, 2],
        )
        self.assertEqual(
            [page["result"]["observation"]["source_page_index"] for page in report["pages"]],
            [0, 1, 2],
        )
        self.assertIn("page 1/3", result.stderr)
        self.assertIn("page 3/3", result.stderr)

    def test_page_failure_stops_without_partial_json(self):
        with tempfile.TemporaryDirectory() as directory:
            result = self.run_document(
                self.compile_document(directory), "--model", "fail-page-1",
            )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, "")
        self.assertIn("controlled page failure", result.stderr)
        self.assertIn("page 2/3", result.stderr)
        self.assertNotIn("page 3/3", result.stderr)


if __name__ == "__main__":
    unittest.main()
