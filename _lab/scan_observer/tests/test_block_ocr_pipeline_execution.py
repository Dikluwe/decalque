import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

from PIL import Image, ImageDraw


ROOT = pathlib.Path(__file__).parents[3]
SCRIPT = ROOT / "_lab/scan_observer" / "block_ocr_pipeline.py"


def load_module():
    spec = importlib.util.spec_from_file_location("block_pipeline", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class BlockOcrPipelineExecutionTests(unittest.TestCase):
    def test_format_block_executes_through_dynamic_loader(self):
        module = load_module()
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "source.png"
            Image.new("RGB", (754, 1044), "white").save(source)
            work = root / "format"
            work.mkdir()
            produced = module.normalize_format(
                {"image": str(source)},
                {"width_pt": 452.18, "height_pt": 626.31, "side": "right"}, work,
            )
            report = json.loads(pathlib.Path(produced["transform"]).read_text())
            self.assertEqual(report["format"], "Book-160x220")
            with Image.open(produced["image"]) as normalized:
                self.assertEqual(normalized.size, (756, 1040))

    def test_executes_dependencies_not_declaration_order(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            image = root / "source.png"
            canvas = Image.new("L", (80, 60), 255)
            ImageDraw.Draw(canvas).rectangle((15, 20, 55, 30), fill=0)
            canvas.save(image)
            config = {
                "version": 1,
                "inputs": {"page": str(image)},
                "blocks": [
                    {"id": "ink", "type": "segment-ink",
                     "inputs": {"image": "normalize.image"}},
                    {"id": "normalize", "type": "normalize-page",
                     "inputs": {"image": "input:page"}},
                ],
            }
            pipeline = root / "pipeline.json"
            pipeline.write_text(json.dumps(config))
            result = subprocess.run(
                [sys.executable, str(SCRIPT), str(pipeline), "--output", str(root / "run")],
                cwd=ROOT, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            report = json.loads(result.stdout)
            self.assertEqual(report["order"], ["normalize", "ink"])
            regions = json.loads((root / "run" / "ink" / "regions.json").read_text())
            self.assertGreaterEqual(len(regions["regions"]), 1)
            self.assertTrue((root / "run" / "manifest.json").is_file())

    def test_dry_run_rejects_cycle_without_creating_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            config = {
                "version": 1, "inputs": {}, "blocks": [
                    {"id": "a", "type": "normalize-page", "inputs": {"image": "b.image"}},
                    {"id": "b", "type": "normalize-page", "inputs": {"image": "a.image"}},
                ],
            }
            pipeline = root / "cycle.json"
            pipeline.write_text(json.dumps(config))
            output = root / "run"
            result = subprocess.run(
                [sys.executable, str(SCRIPT), str(pipeline), "--output", str(output), "--dry-run"],
                cwd=ROOT, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 2)
            self.assertIn("ciclo", result.stderr)
            self.assertFalse(output.exists())

    def test_ovis_chain_contract_resolves_expected_order(self):
        module = load_module()
        config = json.loads((ROOT / "_lab/scan_observer/config/ovis-first-page.json").read_text())
        self.assertEqual(module.validate(config),
                         ["format", "normalize", "observe_raw", "observe_ink",
                          "project_visuals"])


if __name__ == "__main__":
    unittest.main()
