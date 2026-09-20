import importlib.util
import pathlib
import sys
import unittest

from PIL import Image


ROOT = pathlib.Path(__file__).parents[3]
MODULE_PATH = ROOT / "_lab/scan_observer" / "page_format_normalizer.py"


def load_module():
    spec = importlib.util.spec_from_file_location("page_format_normalizer", MODULE_PATH)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class PageFormatNormalizerExecutionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.module = load_module()

    def test_altov_mixture_snaps_to_160x220_without_resampling(self):
        image = Image.new("L", (754, 1044), 255)
        image.putpixel((0, 500), 0)
        normalized, report = self.module.normalize(image, 452.18, 626.31, side="right")
        self.assertEqual(report["format"], "Book-160x220")
        self.assertEqual(report["status"], "normalized")
        self.assertFalse(report["resampled"])
        self.assertEqual(normalized.size, (756, 1040))
        self.assertEqual(report["translation_px"], [0, -2])
        self.assertEqual(normalized.getpixel((0, 498)), 0)

    def test_left_page_places_width_difference_at_outer_left(self):
        image = Image.new("L", (754, 1044), 255)
        image.putpixel((753, 500), 0)
        normalized, report = self.module.normalize(image, 452.18, 626.31, side="left")
        self.assertEqual(report["translation_px"], [2, -2])
        self.assertEqual(normalized.getpixel((755, 498)), 0)

    def test_unknown_format_preserves_canvas(self):
        image = Image.new("RGB", (500, 500), "white")
        normalized, report = self.module.normalize(image, 300, 300)
        self.assertEqual(report["status"], "unknown")
        self.assertEqual(normalized.size, image.size)
        self.assertEqual(report["translation_px"], [0, 0])

    def test_landscape_orientation_is_recognized(self):
        report = self.module.identify(297 * self.module.PT_PER_MM,
                                      210 * self.module.PT_PER_MM)
        self.assertEqual((report["format"], report["orientation"]),
                         ("ISO-A4", "landscape"))


if __name__ == "__main__":
    unittest.main()
