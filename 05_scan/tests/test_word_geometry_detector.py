import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "word_geometry_detector.py"
SPEC = importlib.util.spec_from_file_location("word_geometry_detector", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class InkSegmentsTests(unittest.TestCase):
    def test_baseline_uses_dominant_ink_bottom_and_ignores_descenders(self):
        ink = [[False] * 10 for _ in range(7)]
        for x in range(1, 9):
            bottom = 5 if x in (3, 7) else 3
            for y in range(1, bottom + 1):
                ink[y][x] = True

        baseline, confidence = MODULE.estimate_baseline(ink, 1, 9, offset_y=20)

        self.assertEqual(baseline, 24)
        self.assertEqual(confidence, 0.75)

    def test_baseline_without_ink_is_unknown(self):
        self.assertEqual(MODULE.estimate_baseline([[False] * 3], 0, 3), (None, 0.0))

    def test_groups_character_components_and_splits_only_at_word_gap(self):
        ink = [[False] * 18 for _ in range(5)]
        for x in (1, 2, 4, 5, 11, 12, 14, 15):
            for y in range(1, 4):
                ink[y][x] = True

        boxes = MODULE.ink_segments(ink, 100, 200, minimum_word_gap=4)

        self.assertEqual(boxes, [[101, 201, 106, 204], [111, 201, 116, 204]])

    def test_gap_on_threshold_is_a_word_boundary(self):
        ink = [[True, True, False, False, False, True, True]]
        self.assertEqual(
            MODULE.ink_segments(ink, 0, 0, minimum_word_gap=3),
            [[0, 0, 2, 1], [5, 0, 7, 1]],
        )

    def test_empty_or_invalid_input_has_no_segments(self):
        self.assertEqual(MODULE.ink_segments([], 0, 0, 2), [])
        self.assertEqual(MODULE.ink_segments([[False, False]], 0, 0, 2), [])
        self.assertEqual(MODULE.ink_segments([[True]], 0, 0, 0), [])

    def test_count_mismatch_preserves_unknown_without_partial_assignment(self):
        line = {"id": 3, "text": "duas palavras"}
        MODULE.build_word_segments(line, [[0, 0, 20, 10]], minimum_gap=4)
        self.assertEqual(line["word_geometry_status"], "unknown")
        self.assertEqual(line["word_segments"], [])

    def test_matching_count_names_observed_segments(self):
        line = {"id": 3, "text": "duas palavras"}
        MODULE.build_word_segments(
            line, [[0, 0, 20, 10], [30, 0, 60, 10]], minimum_gap=4
        )
        self.assertEqual(line["word_geometry_status"], "observed")
        self.assertEqual([item["text"] for item in line["word_segments"]], ["duas", "palavras"])
        self.assertEqual(line["word_segments"][1]["bbox"], [30, 0, 60, 10])


if __name__ == "__main__":
    unittest.main()
