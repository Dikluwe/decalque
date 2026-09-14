import copy
import importlib.util
import pathlib
import unittest


MODULE_PATH = pathlib.Path(__file__).parents[1] / "candidate_font_matcher.py"
SPEC = importlib.util.spec_from_file_location("candidate_font_matcher", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def token(text):
    return {
        "kind": "word",
        "text": text,
        "font": {"status": "unknown"},
        "bbox": None,
    }


def page(*tokens):
    return {"regions": [{"lines": [{"tokens": list(tokens)}]}]}


def glyph(text, ref="F1", base="ABCDEF+NimbusRoman-Regular", size=12.0):
    return {
        "text": text,
        "font_ref": ref,
        "base_font": base,
        "font_size_pt": size,
        "position": [0, 10],
    }


class CandidateFontMatcherTests(unittest.TestCase):
    def test_infers_declared_font_for_fully_aligned_token_without_changing_geometry(self):
        source = page(token("Olá"))
        original = copy.deepcopy(source)
        output = MODULE.enrich_page(
            source, {"glyphs": [glyph("O"), glyph("l"), glyph("á")]}, "candidate.pdf"
        )
        result = output["regions"][0]["lines"][0]["tokens"][0]

        self.assertEqual(result["font"]["family"], "NimbusRoman")
        self.assertEqual(result["font"]["size_pt"], 12.0)
        self.assertEqual(result["font"]["status"], "inferred")
        self.assertEqual(result["font"]["evidence"][0]["declared_base_font"], "ABCDEF+NimbusRoman-Regular")
        self.assertEqual(result["bbox"], original["regions"][0]["lines"][0]["tokens"][0]["bbox"])

    def test_ocr_error_keeps_font_unknown(self):
        output = MODULE.enrich_page(
            page(token("paráfo")),
            {"glyphs": [glyph(character) for character in "parágrafo"]},
            "candidate.pdf",
        )
        result = output["regions"][0]["lines"][0]["tokens"][0]
        self.assertEqual(result["font"]["status"], "unknown")

    def test_separates_pdf_encoding_suffix_from_family_and_style(self):
        result = MODULE.font_hypothesis(
            glyph("A", base="TARKWA+LibertinusSerif-Bold-Identity-H", size=24),
            "candidate.pdf",
        )
        self.assertEqual(result["family"], "LibertinusSerif")
        self.assertEqual(result["style"], "normal")
        self.assertEqual(result["weight"], "bold")
        self.assertEqual(
            result["evidence"][0]["declared_base_font"],
            "TARKWA+LibertinusSerif-Bold-Identity-H",
        )

    def test_mixed_fonts_keep_token_unknown(self):
        output = MODULE.enrich_page(
            page(token("texto")),
            {"glyphs": [glyph("t"), glyph("e"), glyph("x", ref="F2"), glyph("t"), glyph("o")]},
            "candidate.pdf",
        )
        self.assertEqual(
            output["regions"][0]["lines"][0]["tokens"][0]["font"]["status"],
            "unknown",
        )

    def test_repeated_token_with_different_candidate_fonts_is_ambiguous(self):
        output = MODULE.enrich_page(
            page(token("eco")),
            {"glyphs": [glyph(c) for c in "eco"] + [glyph(c, ref="F2", base="Other-Bold") for c in "eco"]},
            "candidate.pdf",
        )
        self.assertEqual(
            output["regions"][0]["lines"][0]["tokens"][0]["font"]["status"],
            "unknown",
        )

    def test_monodirectional_rtl_run_is_aligned_in_logical_order(self):
        visual_order = "ملاعلاب ابحرم"
        output = MODULE.enrich_page(
            page(token("مرحبا"), token("بالعالم")),
            {
                "glyphs": [
                    glyph(character, base="ABCDEF+NotoNaskhArabic-Bold")
                    for character in visual_order
                ]
            },
            "candidate.pdf",
        )
        words = output["regions"][0]["lines"][0]["tokens"]
        self.assertEqual([word["font"]["status"] for word in words], ["inferred", "inferred"])
        self.assertEqual(words[0]["font"]["family"], "NotoNaskhArabic")

    def test_font_without_base_font_stays_unknown(self):
        output = MODULE.enrich_page(
            page(token("😀")),
            {"glyphs": [glyph("😀", base=None)]},
            "candidate.pdf",
        )
        self.assertEqual(
            output["regions"][0]["lines"][0]["tokens"][0]["font"]["status"],
            "unknown",
        )


if __name__ == "__main__":
    unittest.main()
