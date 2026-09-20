import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "paddle_line_detector.py"
SPEC = importlib.util.spec_from_file_location("paddle_line_detector", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class NormalizeLineDetectorTests(unittest.TestCase):
    def test_preserves_real_line_polygons_and_separate_recognition_confidence(self):
        output = MODULE.normalize_result(
            {
                "res": {
                    "page_index": 2,
                    "rec_texts": ["linha um", "linha dois"],
                    "rec_scores": [0.91, 0.82],
                    "rec_polys": [
                        [[10, 20], [90, 20], [90, 30], [10, 30]],
                        [[10, 40], [95, 40], [95, 50], [10, 50]],
                    ],
                    "rec_boxes": [[10, 20, 90, 30], [10, 40, 95, 50]],
                }
            }
        )

        self.assertEqual(output["page_index"], 2)
        self.assertEqual(output["lines"][1]["text"], "linha dois")
        self.assertEqual(output["lines"][1]["bbox"], [10, 40, 95, 50])
        self.assertEqual(output["lines"][1]["recognition_confidence"], 0.82)
        self.assertNotIn("word_boxes", output)

    def test_rejects_provider_output_with_truncated_geometry(self):
        with self.assertRaisesRegex(ValueError, "tamanhos diferentes"):
            MODULE.normalize_result(
                {
                    "res": {
                        "rec_texts": ["uma linha"],
                        "rec_scores": [0.9],
                        "rec_polys": [],
                        "rec_boxes": [[0, 0, 10, 10]],
                    }
                }
            )


if __name__ == "__main__":
    unittest.main()
