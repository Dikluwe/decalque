import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).parents[1]
SPEC = importlib.util.spec_from_file_location(
    "editorial_region_classifier", ROOT / "editorial_region_classifier.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def line(x0, y0, x1, text="words"):
    return {"bbox": [x0, y0, x1, y0 + 20], "text": text}


def region(region_id, kind, bbox, lines, style):
    return {
        "id": region_id, "type": kind, "bbox": bbox, "lines": lines,
        "font_style_summary": {"status": "observed", "dominant": style,
                               "slant": "italic" if "italic" in style else "normal",
                               "weight": "regular"},
    }


class EditorialRegionClassifierTests(unittest.TestCase):
    def setUp(self):
        self.page = {"width_px": 1000, "height_px": 1600}

    def classify(self, item):
        return MODULE.classify_region(item, self.page, [])

    def test_regular_short_indented_upper_block_is_epigraph(self):
        item = region("R1", "epigraph", [120, 220, 580, 245],
                      [line(120, 220, 580, '"In the beginning..."')], "regular")
        self.assertEqual(self.classify(item)["role"], "epigraph")

    def test_short_varied_italic_lines_form_verse_or_title_list(self):
        lines = [line(115, 400 + index * 30, right, text)
                 for index, (right, text) in enumerate([
                     (430, "Inventing Methods"), (510, "Through Knowledge"),
                     (270, "Step-by-Step"), (620, "An Alloy Made of Logic"),
                     (390, "Over the Barriers")])]
        item = region("R2", "list", [115, 400, 620, 550], lines, "italic")
        result = self.classify(item)
        self.assertEqual(result["role"], "verse_or_title_list")
        self.assertIn("italic-short-variable-lines", result["rules"])

    def test_long_italic_prose_in_middle_is_block_quote(self):
        lines = [line(100, 500 + index * 30, 900, "continuous prose") for index in range(5)]
        item = region("R3", "paragraph", [100, 500, 900, 650], lines, "italic")
        self.assertEqual(self.classify(item)["role"], "block_quote")

    def test_bottom_italic_prose_with_biographical_signals_is_note(self):
        lines = [line(110, 1250 + index * 30, 910,
                      "Simon Litvin is Vice President and author of books") for index in range(4)]
        item = region("R4", "note", [110, 1250, 910, 1370], lines, "italic")
        result = self.classify(item)
        self.assertEqual(result["role"], "biographical_note")
        self.assertIn("biographical-text-support", result["rules"])

    def test_regular_filled_region_is_body_paragraph(self):
        lines = [line(70, 300 + index * 30, 930, "body prose") for index in range(8)]
        item = region("R5", "paragraph", [70, 300, 930, 540], lines, "regular")
        self.assertEqual(self.classify(item)["role"], "body_paragraph")

    def test_visual_is_preserved_without_text(self):
        item = {"id": "R6", "type": "visual", "bbox": [20, 20, 300, 400], "lines": []}
        self.assertEqual(self.classify(item)["role"], "visual")

    def test_many_lines_at_page_top_are_not_heading(self):
        lines = [line(70, 40 + index * 30, 930, "continuing body") for index in range(10)]
        item = region("R7", "heading", [70, 40, 930, 340], lines, "regular")
        self.assertEqual(self.classify(item)["role"], "body_paragraph")

    def test_short_bold_block_is_figure_caption(self):
        lines = [line(80, 420 + index * 22, 300, "Figure caption") for index in range(3)]
        item = region("R8", "list", [80, 420, 300, 486], lines, "bold")
        item["font_style_summary"]["weight"] = "bold"
        self.assertEqual(self.classify(item)["role"], "figure_caption")

    def test_narrow_regular_column_remains_body(self):
        lines = [line(70, 700 + index * 25, 370, "narrow body") for index in range(8)]
        item = region("R9", "paragraph", [70, 700, 370, 900], lines, "regular")
        self.assertEqual(self.classify(item)["role"], "body_paragraph")


if __name__ == "__main__":
    unittest.main()
