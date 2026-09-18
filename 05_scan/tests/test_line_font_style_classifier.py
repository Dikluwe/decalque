import importlib.util
import pathlib
import sys
import unittest

from PIL import Image, ImageDraw, ImageFont


ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT))
SPEC = importlib.util.spec_from_file_location("line_font_style_classifier", ROOT / "line_font_style_classifier.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

REGULAR = pathlib.Path("/tmp/decalque-sling/Sling.ttf")
LIGHT = pathlib.Path("/tmp/decalque-sling/SlingLight.ttf")


@unittest.skipUnless(REGULAR.exists() and LIGHT.exists(), "requires personal Sling test fonts")
class LineFontStyleClassifierTests(unittest.TestCase):
    def fixture(self, style):
        mask = MODULE.render_candidate_mask("Innovation method", REGULAR, 42, style)
        canvas = Image.new("L", (mask.width + 12, mask.height + 8), 255)
        canvas.paste(Image.eval(mask, lambda value: 255 - value), (6, 4))
        return canvas

    def test_separates_regular_italic_and_bold_with_identical_text_and_box(self):
        for expected in ("regular", "italic", "bold"):
            with self.subTest(expected=expected):
                result = MODULE.classify_crop(
                    self.fixture(expected), "Innovation method", REGULAR, LIGHT
                )
                self.assertEqual(result["status"], "observed")
                self.assertEqual(result["style_class"], expected)
                self.assertIn("candidate_scores", result)
                self.assertEqual(
                    result["slant_measurement"]["style"],
                    "italic" if expected == "italic" else "normal",
                )

    def test_blank_crop_stays_unknown(self):
        result = MODULE.classify_crop(Image.new("L", (100, 20), 255), "text", REGULAR, LIGHT)
        self.assertEqual(result["status"], "unknown")

    def test_region_consensus_prevents_line_by_line_weight_changes(self):
        lines = [
            self.evidence("regular", regular=.10, bold=.16, light=.18),
            self.evidence("bold", regular=.20, bold=.18, light=.24),
            self.evidence("light", regular=.19, bold=.23, light=.17),
            self.evidence("regular", regular=.11, bold=.15, light=.17),
        ]
        summary = MODULE.consolidate_region_styles(lines)
        self.assertEqual(summary["weight"], "regular")
        self.assertTrue(all(line["font_style"]["weight"] == "regular" for line in lines))
        self.assertEqual({line["font_style"]["style_class"] for line in lines}, {"regular"})
        self.assertIn("line_evidence", lines[1]["font_style"])

    def test_region_accepts_bold_only_when_support_is_repeated_and_strong(self):
        lines = [
            self.evidence("bold", regular=.20, bold=.12, light=.24),
            self.evidence("bold", regular=.19, bold=.11, light=.23),
            self.evidence("bold", regular=.21, bold=.14, light=.25),
            self.evidence("regular", regular=.15, bold=.16, light=.22),
        ]
        summary = MODULE.consolidate_region_styles(lines)
        self.assertEqual(summary["weight"], "bold")
        self.assertTrue(all(line["font_style"]["weight"] == "bold" for line in lines))

    def test_region_consolidates_italic_vote_across_all_lines(self):
        lines = [
            self.evidence("regular", italic=True),
            self.evidence("light", italic=True),
            self.evidence("regular", italic=True),
            self.evidence("regular", italic=False),
        ]
        summary = MODULE.consolidate_region_styles(lines)
        self.assertEqual(summary["slant"], "italic")
        self.assertEqual({line["font_style"]["style"] for line in lines}, {"italic"})

    def test_unknown_line_inside_observed_region_inherits_consensus(self):
        lines = [
            self.evidence("regular"),
            {"font_style": {"status": "unknown",
                            "reason": "candidate-margin-insufficient"}},
            self.evidence("regular"),
        ]
        summary = MODULE.consolidate_region_styles(lines)
        inherited = lines[1]["font_style"]
        self.assertEqual(inherited["status"], "inferred")
        self.assertEqual(inherited["style_class"], "regular")
        self.assertEqual(inherited["inference_source"], "region-consensus")
        self.assertEqual(inherited["line_evidence"]["status"], "unknown")
        self.assertEqual(summary["inferred_lines"], 1)

    def test_fully_unknown_region_remains_unknown(self):
        lines = [{"font_style": {"status": "unknown", "reason": "ink-absent"}}]
        summary = MODULE.consolidate_region_styles(lines)
        self.assertEqual(summary["status"], "unknown")
        self.assertEqual(lines[0]["font_style"]["status"], "unknown")

    def test_isolated_dirt_does_not_shift_line_tabulation(self):
        image = Image.new("L", (300, 90), 255)
        draw = ImageDraw.Draw(image)
        font = ImageFont.truetype(str(REGULAR), 18)
        for y, text in ((8, "First aligned line"), (34, "An Alloy Made of Logic"),
                        (60, "Third aligned line")):
            draw.text((80, y), text, font=font, fill=0)
        draw.ellipse((21, 42, 24, 45), fill=0)
        lines = [
            {"bbox": [78, 7, 250, 29]},
            {"bbox": [19, 33, 275, 56]},
            {"bbox": [79, 59, 250, 82]},
        ]
        repaired = MODULE.repair_region_line_boxes(image, lines)
        self.assertEqual(repaired, 1)
        self.assertEqual(lines[1]["bbox_original"], [19, 33, 275, 56])
        self.assertGreaterEqual(lines[1]["bbox"][0], 78)
        self.assertEqual(lines[1]["bbox_repair"]["reason"], "isolated-left-ink")

    def test_region_bbox_is_recomputed_after_line_cleanup(self):
        region = {
            "bbox": [19, 33, 275, 108],
            "lines": [
                {"bbox": [78, 33, 250, 56]},
                {"bbox": [79, 59, 260, 82]},
                {"bbox": [78, 85, 245, 108]},
            ],
        }
        changed = MODULE.recompute_text_region_bbox(region)
        self.assertTrue(changed)
        self.assertEqual(region["bbox_original"], [19, 33, 275, 108])
        self.assertEqual(region["bbox"], [78, 33, 260, 108])
        self.assertEqual(region["bbox_repair"]["reason"], "recomputed-from-clean-lines")

    @staticmethod
    def evidence(weight, *, regular=.15, bold=.17, light=.18, italic=False):
        return {"font_style": {
            "status": "observed", "weight": weight,
            "style": "italic" if italic else "normal",
            "style_class": f"{weight}-italic" if italic and weight != "regular"
                           else "italic" if italic else weight,
            "candidate_scores": {"light": light, "regular": regular, "bold": bold},
            "slant_measurement": {"style": "italic" if italic else "normal"},
        }}


if __name__ == "__main__":
    unittest.main()
