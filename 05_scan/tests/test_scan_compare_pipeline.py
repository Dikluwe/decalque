import importlib.util
import pathlib
import subprocess
import sys
import unittest


ROOT = pathlib.Path(__file__).parents[1]
sys.path.insert(0, str(ROOT))
SPEC = importlib.util.spec_from_file_location("scan_compare_pipeline", ROOT / "scan_compare_pipeline.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ScanComparePipelineTests(unittest.TestCase):
    def test_execucao_help_nao_carrega_provedores_pesados(self):
        output = subprocess.run(
            [sys.executable, str(ROOT / "scan_compare_pipeline.py"), "--help"],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(output.returncode, 0)
        self.assertIn("candidate", output.stdout)

    def test_composes_all_stages_and_returns_comparison(self):
        def vlm(*_):
            return [{"width": 100, "height": 100, "regions": [{"text": "casa", "bbox": [0, 0, 50, 20], "lines": []}]}]

        def lines(*_):
            return [{"lines": [{"id": 0, "text": "casa", "bbox": [10, 5, 30, 15], "polygon": [], "recognition_confidence": 1.0}]}]

        def words(page, _image):
            page["regions"][0]["detected_lines"][0]["word_segments"] = [{
                "id": "0:0", "text": "casa", "bbox": [10, 5, 30, 15],
                "polygon": None, "baseline_y_px": 10, "baseline_confidence": 1.0,
            }]
            return page

        catalog = {
            "page": {"width_pt": 100, "height_pt": 100},
            "glyphs": [
                {"text": c, "position": [10 + i * 5, 10], "advance": 5, "font_size_pt": 10, "font_ref": "F1", "base_font": "ABCDEF+Example-Regular"}
                for i, c in enumerate("casa")
            ],
        }
        result = MODULE.run_pipeline(
            pathlib.Path("image.png"), pathlib.Path("candidate.pdf"), pathlib.Path("catalog"),
            "url", "model", "cpu", "en", 0,
            vlm_provider=vlm, line_provider=lines, image_loader=lambda _: object(),
            catalog_loader=lambda *_: catalog, word_enricher=words,
        )
        self.assertEqual(result["comparison"]["counts"]["preserved"], 1)
        self.assertEqual(result["comparison"]["coverage"], {"comparable": 1, "total_scan": 1})
        self.assertEqual(result["observation"]["point_transform"]["status"], "inferred")
        profile = result["observation"]["regions"][0]["detected_lines"][0]["word_segments"][0]["typographic_profile"]
        self.assertEqual(profile["status"], "observed")
        self.assertEqual(profile["x_height_pt"], 5)


if __name__ == "__main__":
    unittest.main()
