import http.server
import json
import pathlib
import subprocess
import sys
import tempfile
import threading
import unittest

from PIL import Image


SCRIPT = pathlib.Path(__file__).parents[1] / "dual_lmstudio_ocr.py"


class FakeLmStudio(http.server.BaseHTTPRequestHandler):
    calls = []

    def do_POST(self):
        length = int(self.headers["Content-Length"])
        request = json.loads(self.rfile.read(length))
        self.__class__.calls.append(request)
        model = request["model"]
        text = (
            '<img src="images/bbox_100_200_500_600.jpg" />\nVisible title'
            if model == "ovis-test"
            else "Visible title"
        )
        body = json.dumps({"choices": [{"message": {"content": text}}]}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format, *_args):
        pass


class DualOcrExecutionTests(unittest.TestCase):
    def test_process_calls_both_models_and_writes_exact_crop(self):
        FakeLmStudio.calls = []
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), FakeLmStudio)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                source = root / "page.png"
                assets = root / "assets"
                Image.new("RGB", (100, 200), "white").save(source)
                process = subprocess.run(
                    [
                        sys.executable,
                        str(SCRIPT),
                        str(source),
                        "--asset-dir",
                        str(assets),
                        "--base-url",
                        f"http://127.0.0.1:{server.server_port}/v1",
                        "--ovis-model",
                        "ovis-test",
                        "--paddle-model",
                        "paddle-test",
                    ],
                    capture_output=True,
                    text=True,
                    check=False,
                )
                self.assertEqual(process.returncode, 0, process.stderr)
                output = json.loads(process.stdout)
                self.assertEqual([call["model"] for call in FakeLmStudio.calls], ["ovis-test", "paddle-test"])
                self.assertEqual(output["assets"][0]["bbox"], [10, 40, 50, 120])
                self.assertEqual(output["assets"][0]["crop_bbox"], [8, 37, 52, 121])
                with Image.open(output["assets"][0]["path"]) as crop:
                    self.assertEqual(crop.size, (44, 84))
                self.assertEqual(output["comparison"]["status"], "agreement")
        finally:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    unittest.main()
