import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "paddle_lmstudio_adapter.py"
SPEC = importlib.util.spec_from_file_location("paddle_lmstudio_adapter", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class NormalizeResultTests(unittest.TestCase):
    def test_preserves_geometry_and_joins_confidence_by_order(self):
        raw = {
            "res": {
                "width": 945,
                "height": 567,
                "page_index": 0,
                "parsing_res_list": [
                    {
                        "block_label": "text",
                        "block_content": "corpo",
                        "block_bbox": [68, 168, 836, 208],
                        "block_polygon_points": [[68, 168], [836, 168], [836, 208], [68, 208]],
                        "block_order": 2,
                    },
                    {
                        "block_label": "paragraph_title",
                        "block_content": "Título",
                        "block_bbox": [366, 65, 577, 107],
                        "block_polygon_points": [[366, 65], [577, 65], [577, 107], [366, 107]],
                        "block_order": 1,
                    },
                ],
                "layout_det_res": {
                    "boxes": [
                        {"order": 1, "score": 0.83},
                        {"order": 2, "score": 0.91},
                    ]
                },
            }
        }

        output = MODULE.normalize_result(raw, "paddleocr-vl")

        self.assertEqual(output["coordinate_space"], "image-pixels-ydown")
        self.assertEqual((output["width"], output["height"]), (945, 567))
        self.assertEqual([region["order"] for region in output["regions"]], [1, 2])
        self.assertEqual(output["regions"][0]["text"], "Título")
        self.assertEqual(output["regions"][1]["bbox"], [68, 168, 836, 208])
        self.assertEqual(output["regions"][1]["confidence"], 0.91)

    def test_keeps_missing_confidence_absent_and_missing_order_stable(self):
        raw = {
            "res": {
                "width": 100,
                "height": 200,
                "parsing_res_list": [
                    {"block_content": "sem ordem A", "block_order": None},
                    {"block_content": "ordenado", "block_order": 3},
                    {"block_content": "sem ordem B"},
                ],
                "layout_det_res": {"boxes": [{"order": 4, "score": 0.7}]},
            }
        }

        output = MODULE.normalize_result(raw, "modelo-local")

        self.assertEqual(
            [region["text"] for region in output["regions"]],
            ["ordenado", "sem ordem A", "sem ordem B"],
        )
        self.assertTrue(
            all(region["confidence"] is None for region in output["regions"])
        )
        self.assertIsNone(output["page_index"])


if __name__ == "__main__":
    unittest.main()
