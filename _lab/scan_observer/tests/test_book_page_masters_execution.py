import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).parents[1] / "book_page_masters.py"


class BookPageMastersExecutionTests(unittest.TestCase):
    def run_profile(self, data):
        with tempfile.TemporaryDirectory() as directory:
            path = pathlib.Path(directory) / "pages.json"
            path.write_text(json.dumps(data))
            return subprocess.run([sys.executable, str(SCRIPT), str(path)],
                                  capture_output=True, text=True, check=False)

    def test_position_classifies_spread_even_when_pdf_and_printed_numbers_disagree(self):
        data = {"schema_version": 1,
                "page": {"width_pt": 450, "height_pt": 720,
                         "source_width_px": 900, "source_height_px": 1440},
                "pages": [
                    {"pdf_page": 10, "printed_page": 7,
                     "body_bbox_px": [80, 160, 820, 1300],
                     "pagination": {"text": "7", "bbox_px": [800, 1360, 820, 1390]}},
                    {"pdf_page": 11, "printed_page": 8,
                     "body_bbox_px": [80, 160, 820, 1300],
                     "pagination": {"text": "8", "bbox_px": [80, 1360, 100, 1390]}}
                ]}
        process = self.run_profile(data)
        self.assertEqual(process.returncode, 0, process.stderr)
        result = json.loads(process.stdout)
        self.assertEqual([page["side"] for page in result["classified_pages"]], ["right", "left"])
        self.assertTrue(all(page["classification_evidence"] == "pagination-position"
                            for page in result["classified_pages"]))
        self.assertEqual(result["masters"]["right"]["inner_margin_pt"], 40)
        self.assertEqual(result["masters"]["left"]["inner_margin_pt"], 40)

    def test_printed_parity_then_sequence_are_explicit_fallbacks(self):
        data = {"schema_version": 1,
                "page": {"width_pt": 450, "height_pt": 720,
                         "source_width_px": 900, "source_height_px": 1440},
                "pages": [{"pdf_page": 3, "printed_page": 2}, {"pdf_page": 4}]}
        process = self.run_profile(data)
        self.assertEqual(process.returncode, 0, process.stderr)
        pages = json.loads(process.stdout)["classified_pages"]
        self.assertEqual((pages[0]["side"], pages[0]["classification_evidence"]),
                         ("left", "printed-page-parity"))
        self.assertEqual((pages[1]["side"], pages[1]["classification_evidence"]),
                         ("left", "physical-sequence"))

    def test_invalid_geometry_fails_as_process_contract(self):
        process = self.run_profile({"schema_version": 1, "page": {}, "pages": [{}]})
        self.assertEqual(process.returncode, 2)
        self.assertIn("dimensões", process.stderr)


if __name__ == "__main__":
    unittest.main()
