import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


MATCHER = load("line_geometry_matcher")
ADAPTER = load("paddle_lmstudio_adapter")


def observed(identifier, text, bbox):
    x0, y0, x1, y1 = bbox
    return {
        "id": identifier,
        "text": text,
        "bbox": bbox,
        "polygon": [[x0, y0], [x1, y0], [x1, y1], [x0, y1]],
        "recognition_confidence": 0.9,
    }


class LineGeometryMatcherTests(unittest.TestCase):
    def test_attaches_two_observed_lines_without_copying_box_to_semantic_line(self):
        text = "Primeira linha Segunda linha"
        page = {
            "regions": [
                {
                    "text": text,
                    "bbox": [0, 0, 200, 80],
                    "lines": ADAPTER.derive_lines(text),
                }
            ]
        }
        output = MATCHER.enrich_page(
            page,
            {
                "lines": [
                    observed(4, "Primeira linha", [10, 10, 150, 25]),
                    observed(5, "Segunda linha", [10, 35, 145, 50]),
                ]
            },
        )
        region = output["regions"][0]

        self.assertEqual([line["id"] for line in region["detected_lines"]], [4, 5])
        self.assertIsNone(region["lines"][0]["bbox"])
        words = [token for token in region["lines"][0]["tokens"] if token["kind"] == "word"]
        self.assertEqual([token["line_geometry_ref"] for token in words], [4, None, 5, None])
        self.assertEqual(output["unassigned_lines"], [])

    def test_outside_or_textually_incompatible_line_stays_unassigned(self):
        page = {"regions": [{"text": "texto", "bbox": [0, 0, 20, 20], "lines": []}]}
        outside = observed(1, "texto", [30, 30, 40, 40])
        incompatible = observed(2, "figura", [1, 1, 10, 10])

        output = MATCHER.enrich_page(page, {"lines": [outside, incompatible]})

        self.assertEqual(output["regions"][0]["detected_lines"], [])
        self.assertEqual([line["id"] for line in output["unassigned_lines"]], [1, 2])

    def test_repeated_token_in_two_detected_lines_has_no_implicit_winner(self):
        page = {
            "regions": [
                {
                    "text": "eco eco",
                    "bbox": [0, 0, 100, 50],
                    "lines": ADAPTER.derive_lines("eco eco"),
                }
            ]
        }
        output = MATCHER.enrich_page(
            page,
            {"lines": [observed(1, "eco", [0, 0, 30, 10]), observed(2, "eco", [0, 20, 30, 30])]},
        )
        words = [
            token
            for token in output["regions"][0]["lines"][0]["tokens"]
            if token["kind"] == "word"
        ]
        self.assertEqual([token["line_geometry_ref"] for token in words], [None, None])


if __name__ == "__main__":
    unittest.main()
