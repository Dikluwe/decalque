import importlib.util
import pathlib
import sys
import unittest

import numpy as np


ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT))
SPEC = importlib.util.spec_from_file_location("candidate_raster_profile", ROOT / "candidate_raster_profile.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class CandidateRasterProfileTests(unittest.TestCase):
    def test_unique_candidate_word_receives_profile_from_rendered_ink(self):
        image = np.full((100, 100), 255, dtype=np.uint8)
        image[10:20, 10:30] = 0
        catalog = {
            "page": {"width_pt": 100, "height_pt": 100},
            "glyphs": [
                {"text": c, "position": [10 + i * 5, 20], "advance": 5,
                 "font_size_pt": 10, "font_ref": "F1"}
                for i, c in enumerate("casa")
            ],
        }
        page = {"width": 100, "height": 100, "regions": [{"detected_lines": [{"word_segments": [{"text": "casa"}]}]}]}
        output = MODULE.enrich_page(page, catalog, pathlib.Path("candidate.pdf"), 0, renderer=lambda *_: image)
        profile = output["regions"][0]["detected_lines"][0]["word_segments"][0]["candidate_typographic_profile"]
        self.assertEqual(profile["status"], "observed")
        self.assertEqual(profile["x_height_pt"], 10)

    def test_repeated_candidate_word_is_explicitly_ambiguous(self):
        image = np.full((100, 100), 255, dtype=np.uint8)
        catalog = {"page": {"width_pt": 100, "height_pt": 100}, "glyphs": []}
        page = {"width": 100, "height": 100, "regions": [{"detected_lines": [{"word_segments": [{"text": "eco"}]}]}]}
        output = MODULE.enrich_page(page, catalog, pathlib.Path("candidate.pdf"), 0, renderer=lambda *_: image)
        self.assertEqual(output["regions"][0]["detected_lines"][0]["word_segments"][0]["candidate_typographic_profile"]["status"], "unknown")

    def test_adjacent_line_ink_cannot_inflate_second_line_profile(self):
        image = np.full((50, 50), 255, dtype=np.uint8)
        image[7:20, 10:30] = 0
        image[25:30, 10:30] = 0
        glyphs = []
        for y in (20, 30):
            glyphs.extend(
                {"text": c, "position": [10 + i * 5, y], "advance": 5,
                 "font_size_pt": 10, "font_ref": "F1"}
                for i, c in enumerate("casa")
            )
            glyphs.append({"text": " ", "position": [30, y], "advance": 5,
                           "font_size_pt": 10, "font_ref": "F1"})
        profiles = MODULE.candidate_profiles(
            {"page": {"width_pt": 50, "height_pt": 50}, "glyphs": glyphs},
            image, 50, 50,
        )["casa"]
        self.assertEqual(profiles[1]["x_height_pt"], 5)
