# Crystalline Lineage
# @prompt _lab/scan_observer/specs/book-spread-feedback.md
# @updated 2026-09-15

import importlib.util
import pathlib
import sys
import unittest


ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT))
SPEC = importlib.util.spec_from_file_location("book_spread_feedback", ROOT / "book_spread_feedback.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class BookSpreadFeedbackTests(unittest.TestCase):
    def test_feedback_parameters_materially_change_typst_source(self):
        ocr = {"pages": [{"page_number": 1, "text": "Heading\nBody words here\n1"}]}
        master = {"sample_count": 1, "outer_margin_pt": 20, "inner_margin_pt": 30,
                  "top_margin_pt": 40, "body_width_pt": 140, "first_line_indent_pt": 10,
                  "leading_pt": 14, "pagination_center_pt": [180, 280]}
        profile = {"page": {"width_pt": 200, "height_pt": 300},
                   "masters": {"left": master, "right": master}}
        base = {"size_pt": 10, "title_size_pt": 12, "leading_pt": 14,
                "body_width_pt": 140, "top_pt": 40, "indent_pt": 10,
                "paragraph_spacing_pt": 6, "title_weights": {"1": "regular"}}
        changed = dict(base, size_pt=11, leading_pt=18, body_width_pt=130, top_pt=45,
                       indent_pt=12, paragraph_spacing_pt=0)
        left = MODULE.source_for(ocr, profile, [1], "Liberation Serif", base)
        right = MODULE.source_for(ocr, profile, [1], "Liberation Serif", changed)
        self.assertNotEqual(left, right)
        for witness in ("size: 11pt", "width: 130pt", "dy: 45pt",
                        "first-line-indent: 12pt", "spacing: 0pt"):
            self.assertIn(witness, right)

    def test_title_weight_is_independent_from_body_font(self):
        ocr = {"pages": [{"page_number": 1, "text": "Heading\nBody\n1"}]}
        master = {"sample_count": 1, "outer_margin_pt": 20, "inner_margin_pt": 30,
                  "top_margin_pt": 40, "body_width_pt": 140, "first_line_indent_pt": 10,
                  "leading_pt": 14, "pagination_center_pt": [180, 280]}
        profile = {"page": {"width_pt": 200, "height_pt": 300},
                   "masters": {"left": master, "right": master}}
        params = {"size_pt": 10, "title_size_pt": 13, "leading_pt": 14,
                  "body_width_pt": 140, "top_pt": 40, "indent_pt": 10,
                  "paragraph_spacing_pt": 0, "title_weights": {"1": "bold"}}
        source = MODULE.source_for(ocr, profile, [1], "Suranna", params, "Ancizar Serif")
        self.assertIn('font: "Ancizar Serif", size: 13pt, weight: "bold"', source)
        self.assertIn('font: "Suranna", size: 10pt', source)

    def test_bold_weight_can_be_selected_per_page(self):
        params = {"title_weights": {"13": "regular", "14": "bold"}}
        self.assertEqual(params["title_weights"]["13"], "regular")
        self.assertEqual(params["title_weights"]["14"], "bold")

    def test_mirrored_fallback_places_odd_footer_on_outer_right(self):
        master = {"pagination_center_pt": [20, 280]}
        self.assertEqual(MODULE.book_spread_example.mirrored(master, 200)["pagination_center_pt"],
                         [180, 280])

    def test_source_can_represent_no_extra_paragraph_spacing(self):
        self.assertEqual(MODULE.book_spread_example.num(0.0), "0")


if __name__ == "__main__":
    unittest.main()
