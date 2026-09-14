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
        self.assertEqual(output["regions"][1]["layout_confidence"], 0.91)
        self.assertEqual(output["schema_version"], 2)

    def test_explicit_lines_and_tokens_are_reversible_without_fabricated_geometry(self):
        raw = {
            "res": {
                "width": 100,
                "height": 200,
                "parsing_res_list": [
                    {
                        "block_content": "Olá, mundo!\r\nSegunda linha\n",
                        "block_bbox": [1, 2, 90, 40],
                        "block_order": 1,
                    }
                ],
            }
        }

        region = MODULE.normalize_result(raw, "modelo-local")["regions"][0]
        lines = region["lines"]

        self.assertEqual([line["break_after"] for line in lines], ["crlf", "lf"])
        self.assertEqual("".join(token["text"] for token in lines[0]["tokens"]), "Olá, mundo!")
        breaks = {"crlf": "\r\n", "lf": "\n", "cr": "\r", None: ""}
        reconstructed = "".join(
            "".join(token["text"] for token in line["tokens"])
            + breaks[line["break_after"]]
            for line in lines
        )
        self.assertEqual(reconstructed, region["text"])
        self.assertEqual(lines[0]["tokens"][0]["span"], [0, 3])
        self.assertTrue(all(line["bbox"] is None for line in lines))
        self.assertTrue(
            all(token["bbox"] is None for line in lines for token in line["tokens"])
        )
        self.assertTrue(
            all(
                token["font"] == MODULE.unknown_font()
                for line in lines
                for token in line["tokens"]
            )
        )

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
            all(region["layout_confidence"] is None for region in output["regions"])
        )
        self.assertIsNone(output["page_index"])


if __name__ == "__main__":
    unittest.main()
