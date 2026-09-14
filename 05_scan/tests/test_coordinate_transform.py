import importlib.util
import math
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "coordinate_transform.py"
SPEC = importlib.util.spec_from_file_location("coordinate_transform", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class CoordinateTransformTests(unittest.TestCase):
    def test_converts_region_and_detected_line_without_mutating_pixels(self):
        page = {
            "width": 1200,
            "height": 1600,
            "regions": [
                {
                    "bbox": [100, 200, 1100, 400],
                    "polygon": [[100, 200], [1100, 200], [1100, 400], [100, 400]],
                    "detected_lines": [{"bbox": [120, 220, 900, 260], "polygon": None}],
                    "lines": [{"bbox": None, "polygon": None, "tokens": []}],
                }
            ],
        }
        output = MODULE.enrich_page(
            page, {"page": {"width_pt": 600, "height_pt": 800}}
        )

        self.assertEqual(output["point_transform"]["status"], "inferred")
        self.assertEqual(output["regions"][0]["bbox"], [100, 200, 1100, 400])
        self.assertEqual(output["regions"][0]["bbox_pt"], [50, 100, 550, 200])
        self.assertEqual(
            output["regions"][0]["detected_lines"][0]["bbox_pt"],
            [60, 110, 450, 130],
        )
        self.assertIsNone(output["regions"][0]["lines"][0]["bbox_pt"])

    def test_aspect_mismatch_keeps_all_point_geometry_unknown(self):
        page = {
            "width": 1000,
            "height": 1000,
            "regions": [{"bbox": [0, 0, 100, 100], "polygon": None}],
        }
        output = MODULE.enrich_page(
            page, {"page": {"width_pt": 600, "height_pt": 800}}
        )

        self.assertEqual(output["point_transform"]["status"], "unknown")
        self.assertEqual(output["point_transform"]["reason"], "aspect-ratio-mismatch")
        self.assertIsNone(output["regions"][0]["bbox_pt"])

    def test_invalid_dimensions_never_produce_scale(self):
        for invalid in (None, 0, -1, math.inf, math.nan, True):
            with self.subTest(invalid=invalid):
                transform = MODULE.derive_transform(invalid, 100, 50, 50)
                self.assertEqual(transform["status"], "unknown")
                self.assertEqual(transform["reason"], "invalid-dimensions")

    def test_threshold_boundary_is_inclusive_and_deterministic(self):
        first = MODULE.derive_transform(1005, 1000, 1000, 1000)
        second = MODULE.derive_transform(1005, 1000, 1000, 1000)
        self.assertEqual(first, second)
        self.assertEqual(first["status"], "inferred")


if __name__ == "__main__":
    unittest.main()
