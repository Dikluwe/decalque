import copy
import importlib.util
import pathlib
import unittest

from PIL import Image, ImageDraw


ROOT = pathlib.Path(__file__).parents[1]
SPEC = importlib.util.spec_from_file_location("frozen_typography_profile", ROOT / "frozen_typography_profile.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class FrozenTypographyProfileTests(unittest.TestCase):
    def test_spanning_visual_and_title_end_before_two_column_body(self):
        image = Image.new("L", (600, 800), "white")
        draw = ImageDraw.Draw(image)
        draw.rectangle((50, 40, 550, 240), outline="black", width=5)
        for x in range(70, 530, 20):
            draw.line((x, 50, 300, 230), fill="black", width=2)
        draw.line((50, 280, 550, 280), fill="black", width=3)
        draw.rectangle((210, 300, 390, 316), fill="black")
        draw.line((50, 335, 550, 335), fill="black", width=3)
        for y in range(370, 740, 22):
            draw.rectangle((50, y, 275, y + 8), fill="black")
            draw.rectangle((325, y, 550, y + 8), fill="black")
        observed = MODULE.observe_geometry(image, 11, 450, 600)
        self.assertEqual(len(observed["columns"]), 2)
        self.assertGreaterEqual(observed["columns"][0][1], 330)
        kinds = {zone["kind"] for zone in observed["spanning_zones"]}
        self.assertIn("visual", kinds)
        self.assertIn("heading", kinds)

    def test_two_columns_and_general_metrics_are_aggregated(self):
        observations = [
            {"page": 10, "columns": [[50, 100, 360, 1000], [390, 100, 700, 1000]],
             "line_heights_px": [16, 17, 17], "baseline_steps_px": [20, 20, 21],
             "page_size_px": [750, 1100], "page_size_pt": [461.18, 675.75]},
            {"page": 11, "columns": [[52, 95, 362, 1002], [391, 95, 703, 1002]],
             "line_heights_px": [17, 17, 18], "baseline_steps_px": [20, 21, 20],
             "page_size_px": [750, 1100], "page_size_pt": [461.18, 675.75]},
        ]
        profile = MODULE.freeze_profile(observations, "Nimbus Roman")
        self.assertEqual(profile["state"], "frozen")
        self.assertEqual(profile["page_master"]["column_count"], 2)
        self.assertGreater(profile["classes"]["body"]["font_size_pt"], 0)
        self.assertEqual(profile["classes"]["body"]["horizontal_scale"], 1.0)

    def test_audit_never_mutates_frozen_profile(self):
        profile = MODULE.freeze_profile([{
            "page": 1, "columns": [[10, 10, 90, 190]], "line_heights_px": [10, 10],
            "baseline_steps_px": [14], "page_size_px": [100, 200],
            "page_size_pt": [100, 200]}], "Serif")
        before = copy.deepcopy(profile)
        result = MODULE.audit_observation(profile, {
            "page": 2, "columns": [[10, 10, 90, 190]], "line_heights_px": [20],
            "baseline_steps_px": [30], "page_size_px": [100, 200],
            "page_size_pt": [100, 200]})
        self.assertEqual(profile, before)
        self.assertEqual(result["profile_hash"], profile["profile_hash"])
        self.assertTrue(result["marks"])


if __name__ == "__main__":
    unittest.main()
