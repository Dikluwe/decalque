import importlib.util
import pathlib
import unittest


PATH = pathlib.Path(__file__).parents[1] / "scan_word_compare.py"
SPEC = importlib.util.spec_from_file_location("scan_word_compare", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def glyph(text, x, y=20, advance=5, size=10):
    return {"text": text, "position": [x, y], "advance": advance, "font_size_pt": size, "font_ref": "F1"}


def scan(*segments):
    return {"regions": [{"detected_lines": [{"id": 1, "word_segments": list(segments)}]}]}


def segment(text, bbox, baseline=20, confidence=1.0):
    return {
        "text": text,
        "bbox_pt": bbox,
        "baseline_y_pt": baseline,
        "baseline_confidence": confidence,
    }


class ScanWordCompareTests(unittest.TestCase):
    def test_equal_horizontal_geometry_is_preserved_with_coverage(self):
        report = MODULE.compare(scan(segment("casa", [10, 5, 30, 15])), {"glyphs": [glyph(c, 10 + i * 5) for i, c in enumerate("casa")]})
        self.assertEqual(report["words"][0]["status"], "preserved")
        self.assertEqual(report["words"][0]["vertical_status"], "preserved")
        self.assertEqual(report["coverage"], {"comparable": 1, "total_scan": 1})
        self.assertEqual(report["words"][0]["typography_status"], "unknown")

    def test_shift_beyond_tolerance_is_violated_with_witness(self):
        report = MODULE.compare(scan(segment("casa", [14, 5, 34, 15])), {"glyphs": [glyph(c, 10 + i * 5) for i, c in enumerate("casa")]})
        word = report["words"][0]
        self.assertEqual(word["status"], "violated")
        self.assertEqual(word["deltas_pt"]["start_x"], 4)
        self.assertEqual(word["candidate_span_pt"], [10, 30])

    def test_vertical_shift_beyond_tolerance_is_violated_with_witness(self):
        report = MODULE.compare(
            scan(segment("casa", [10, 5, 30, 15], baseline=24)),
            {"glyphs": [glyph(c, 10 + i * 5) for i, c in enumerate("casa")]},
        )
        word = report["words"][0]
        self.assertEqual(word["horizontal_status"], "preserved")
        self.assertEqual(word["vertical_status"], "violated")
        self.assertEqual(word["deltas_pt"]["baseline_y"], 4)

    def test_low_confidence_baseline_keeps_global_verdict_unknown(self):
        report = MODULE.compare(
            scan(segment("casa", [10, 5, 30, 15], baseline=20, confidence=0.49)),
            {"glyphs": [glyph(c, 10 + i * 5) for i, c in enumerate("casa")]},
        )
        word = report["words"][0]
        self.assertEqual(word["horizontal_status"], "preserved")
        self.assertEqual(word["vertical_status"], "unknown")
        self.assertEqual(word["status"], "unknown")

    def test_repeated_or_missing_geometry_is_unknown(self):
        catalog = {"glyphs": [glyph(c, i * 5) for i, c in enumerate("eco eco")]}
        report = MODULE.compare(scan(segment("eco", None)), catalog)
        self.assertEqual(report["words"][0]["status"], "unknown")
        self.assertEqual(report["coverage"]["comparable"], 0)

    def test_scan_line_spanning_two_candidate_lines_is_reflow_violation(self):
        glyphs = [glyph(c, i * 5, y=10) for i, c in enumerate("um ")]
        glyphs += [glyph(c, i * 5, y=30) for i, c in enumerate("dois")]
        report = MODULE.compare(scan(segment("um", [0, 0, 10, 10]), segment("dois", [0, 20, 20, 30])), {"glyphs": glyphs})
        self.assertEqual(report["lines"][0]["status"], "violated")
        self.assertEqual(report["lines"][0]["candidate_line_ids"], [0, 1])

    def test_monodirectional_rtl_candidate_is_compared_in_logical_order(self):
        glyphs = [glyph(c, 10 + i * 5) for i, c in enumerate("ابحرم")]
        report = MODULE.compare(scan(segment("مرحبا", [10, 5, 35, 15])), {"glyphs": glyphs})
        self.assertEqual(report["words"][0]["status"], "preserved")


if __name__ == "__main__":
    unittest.main()
