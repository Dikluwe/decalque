import importlib.util
import pathlib
import unittest

from PIL import Image, ImageDraw


ROOT = pathlib.Path(__file__).parents[1]
SPEC = importlib.util.spec_from_file_location("page_geometry_normalizer", ROOT / "page_geometry_normalizer.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PageGeometryNormalizerTests(unittest.TestCase):
    def test_recovers_skew_and_preserves_canvas(self):
        base = Image.new("L", (500, 700), "white")
        draw = ImageDraw.Draw(base)
        for y in range(100, 600, 28):
            draw.rectangle((60, y, 440, y + 8), fill="black")
        skewed = base.rotate(1.8, resample=Image.Resampling.BICUBIC, fillcolor="white")
        normalized, report = MODULE.normalize_image(skewed, max_angle=3.0, step=0.1)
        self.assertEqual(normalized.size, skewed.size)
        self.assertAlmostEqual(abs(report["observed_skew_deg"]), 1.8, delta=0.3)
        self.assertEqual(report["perspective_status"], "unknown")
        self.assertEqual(len(report["inverse_affine"]), 6)

    def test_blank_page_stays_unknown_and_unrotated(self):
        image = Image.new("L", (300, 400), "white")
        normalized, report = MODULE.normalize_image(image)
        self.assertEqual(normalized.tobytes(), image.tobytes())
        self.assertEqual(report["status"], "unknown")
        self.assertEqual(report["applied_rotation_deg"], 0.0)


if __name__ == "__main__":
    unittest.main()
