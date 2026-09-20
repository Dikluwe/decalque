# Crystalline Lineage
# @prompt _lab/scan_observer/specs/scan-margin-word-anchors.md
# @updated 2026-09-16

import importlib.util
import pathlib
import sys
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "word_geometry_detector.py"
sys.path.insert(0, str(MODULE_PATH.parent))
SPEC = importlib.util.spec_from_file_location("word_geometry_detector_margin", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def ink_line(width, spans, height=8):
    ink = [[False] * width for _ in range(height)]
    for x0, x1 in spans:
        for x in range(x0, x1):
            for y in range(2, 7):
                ink[y][x] = True
    return ink


class MarginWordAnchorTests(unittest.TestCase):
    def test_anchors_first_and_last_words_without_segmenting_merged_middle(self):
        # "In" | a deliberately merged middle | "title."
        ink = ink_line(80, [(2, 6), (8, 12), (20, 25), (27, 55), (64, 69), (71, 77)])

        result = MODULE.margin_word_anchors(
            ink, "In a merged middle title.", [100, 200, 180, 208], minimum_word_gap=7
        )

        self.assertEqual(result["margin_anchor_status"], "observed")
        self.assertEqual(result["left"]["text"], "In")
        self.assertEqual(result["left"]["bbox"], [102, 202, 112, 207])
        self.assertEqual(result["right"]["text"], "title.")
        self.assertEqual(result["right"]["bbox"], [164, 202, 177, 207])
        self.assertEqual(result["anchored_text_bbox"], [102, 200, 177, 208])

    def test_one_word_line_has_coincident_left_and_right_anchors(self):
        result = MODULE.margin_word_anchors(
            ink_line(40, [(7, 12), (14, 22)]), "Algorithm", [10, 20, 50, 28],
            minimum_word_gap=7,
        )

        self.assertEqual(result["margin_anchor_status"], "observed")
        self.assertEqual(result["left"]["bbox"], result["right"]["bbox"])
        self.assertEqual(result["anchored_text_bbox"], [17, 20, 32, 28])

    def test_missing_edge_gap_stays_unknown_for_multiword_line(self):
        result = MODULE.margin_word_anchors(
            ink_line(50, [(2, 48)]), "two words", [0, 0, 50, 8], minimum_word_gap=7
        )

        self.assertEqual(result, {"margin_anchor_status": "unknown"})

    def test_empty_text_or_ink_stays_unknown(self):
        self.assertEqual(
            MODULE.margin_word_anchors([[False] * 10], "word", [0, 0, 10, 1], 3),
            {"margin_anchor_status": "unknown"},
        )
        self.assertEqual(
            MODULE.margin_word_anchors([[True] * 10], "", [0, 0, 10, 1], 3),
            {"margin_anchor_status": "unknown"},
        )


if __name__ == "__main__":
    unittest.main()
