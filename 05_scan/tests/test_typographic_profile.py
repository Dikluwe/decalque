import importlib.util
import pathlib
import unittest


PATH = pathlib.Path(__file__).parents[1] / "typographic_profile.py"
SPEC = importlib.util.spec_from_file_location("typographic_profile", PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def segment(text, baseline=20, confidence=1.0):
    return {
        "text": text,
        "bbox": [0, 5, 20, 23],
        "bbox_pt": [0, 2.5, 10, 11.5],
        "baseline_y_px": baseline,
        "baseline_y_pt": baseline / 2,
        "baseline_confidence": confidence,
    }


class TypographicProfileTests(unittest.TestCase):
    def test_shape_descriptor_is_normalized_and_deterministic(self):
        ink = [
            [True, False, False, False],
            [True, True, False, False],
            [False, True, False, False],
            [False, True, True, True],
        ]
        first = MODULE.ink_shape_descriptor(ink, bins=2)
        self.assertEqual(first, MODULE.ink_shape_descriptor(ink, bins=2))
        self.assertEqual(first["density"], 7 / 16)
        self.assertEqual(len(first["horizontal_projection"]), 2)
        self.assertEqual(len(first["occupancy"]), 8 * 16)

    def test_shape_descriptor_without_ink_is_unknown(self):
        self.assertIsNone(MODULE.ink_shape_descriptor([[False, False]]))

    def test_x_height_only_word_exposes_x_height_without_other_claims(self):
        profile = MODULE.profile_for_segment(segment("casa"))
        self.assertEqual(profile["status"], "observed")
        self.assertEqual(profile["x_height_px"], 15)
        self.assertEqual(profile["x_height_pt"], 7.5)
        self.assertIsNone(profile["ascender_height_px"])
        self.assertIsNone(profile["descender_depth_px"])

    def test_ascender_and_descender_are_reported_only_when_text_supports_them(self):
        profile = MODULE.profile_for_segment(segment("play"))
        self.assertEqual(profile["ascender_height_px"], 15)
        self.assertEqual(profile["descender_depth_px"], 3)
        self.assertIsNone(profile["x_height_px"])

    def test_accented_word_does_not_mistake_accent_top_for_x_height(self):
        profile = MODULE.profile_for_segment(segment("ação"))
        self.assertTrue(profile["features"]["has_marks"])
        self.assertIsNone(profile["x_height_px"])

    def test_uppercase_word_is_ascender_evidence_not_x_height(self):
        profile = MODULE.profile_for_segment(segment("O"))
        self.assertEqual(profile["ascender_height_px"], 15)
        self.assertIsNone(profile["x_height_px"])

    def test_low_confidence_or_inconsistent_baseline_stays_unknown(self):
        self.assertEqual(
            MODULE.profile_for_segment(segment("casa", confidence=0.49))["status"],
            "unknown",
        )
        self.assertEqual(
            MODULE.profile_for_segment(segment("casa", baseline=30))["status"],
            "unknown",
        )

    def test_enrichment_does_not_mutate_input(self):
        source = {"regions": [{"detected_lines": [{"word_segments": [segment("casa")]}]}]}
        output = MODULE.enrich_page(source)
        self.assertNotIn("typographic_profile", source["regions"][0]["detected_lines"][0]["word_segments"][0])
        self.assertEqual(
            output["regions"][0]["detected_lines"][0]["word_segments"][0]["typographic_profile"]["status"],
            "observed",
        )


if __name__ == "__main__":
    unittest.main()
