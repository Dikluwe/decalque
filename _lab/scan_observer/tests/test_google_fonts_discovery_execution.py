import http.server
import json
import os
import pathlib
import subprocess
import sys
import tempfile
import threading
import unittest
import urllib.parse

from PIL import Image, ImageDraw, ImageFont


SCRIPT = pathlib.Path(__file__).parents[1] / "google_fonts_discovery.py"
SERIF = pathlib.Path("/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf")
SERIF_ITALIC = pathlib.Path("/usr/share/fonts/truetype/liberation/LiberationSerif-Italic.ttf")
SANS = pathlib.Path("/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf")
if not SANS.exists():
    SANS = pathlib.Path("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf")


class GoogleFontsStub(http.server.BaseHTTPRequestHandler):
    query = None

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/webfonts/v1/webfonts":
            self.__class__.query = urllib.parse.parse_qs(parsed.query)
            base = f"http://127.0.0.1:{self.server.server_port}"
            body = json.dumps({"items": [
                {"family": "Wrong Sans", "category": "sans-serif", "files": {"700": base + "/sans.ttf", "italic": base + "/sans.ttf"}},
                {"family": "Exact Serif", "category": "serif", "files": {"700": base + "/serif.ttf", "italic": base + "/serif-italic.ttf"}},
                {"family": "Missing Bold", "category": "serif", "files": {"regular": base + "/serif.ttf"}},
            ]}).encode()
        elif parsed.path == "/serif.ttf":
            body = SERIF.read_bytes()
        elif parsed.path == "/sans.ttf":
            body = SANS.read_bytes()
        elif parsed.path == "/serif-italic.ttf":
            body = SERIF_ITALIC.read_bytes()
        else:
            self.send_error(404)
            return
        self.send_response(200)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, _format, *_args):
        pass


class StalledFontStub(GoogleFontsStub):
    def do_GET(self):
        if urllib.parse.urlparse(self.path).path == "/stalled.ttf":
            threading.Event().wait(1)
            return
        if urllib.parse.urlparse(self.path).path == "/webfonts/v1/webfonts":
            base = f"http://127.0.0.1:{self.server.server_port}"
            body = json.dumps({"items": [
                {"family": "Stalled", "category": "serif", "files": {"700": base + "/stalled.ttf"}},
                {"family": "Working", "category": "serif", "files": {"700": base + "/serif.ttf"}},
            ]}).encode()
            self.send_response(200)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        super().do_GET()


class GoogleFontsDiscoveryExecutionTests(unittest.TestCase):
    def test_loads_key_from_protected_user_config_without_exposing_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            secret_file = root / ".config/decalque/secrets.env"
            secret_file.parent.mkdir(parents=True)
            secret_file.write_text("export GOOGLE_FONTS_API_KEY=test-protected-key\n")
            secret_file.chmod(0o600)
            process = subprocess.run([
                sys.executable, str(SCRIPT), "/does/not/matter.png", "--text", "Innovation",
                "--api-url", "http://127.0.0.1:1/webfonts/v1/webfonts",
                "--allow-insecure-localhost",
            ], capture_output=True, text=True, check=False,
               env={"HOME": str(root), "PATH": os.environ.get("PATH", "")})
            self.assertEqual(process.returncode, 2)
            self.assertIn("falha ao consultar catálogo", process.stderr)
            self.assertNotIn("test-protected-key", process.stdout + process.stderr)

    def test_rejects_user_config_readable_by_other_users(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            secret_file = root / ".config/decalque/secrets.env"
            secret_file.parent.mkdir(parents=True)
            secret_file.write_text("GOOGLE_FONTS_API_KEY=test-protected-key\n")
            secret_file.chmod(0o644)
            process = subprocess.run([
                sys.executable, str(SCRIPT), "/does/not/matter.png", "--text", "Innovation",
            ], capture_output=True, text=True, check=False,
               env={"HOME": str(root), "PATH": os.environ.get("PATH", "")})
            self.assertEqual(process.returncode, 2)
            self.assertIn("permissões inseguras", process.stderr)
            self.assertNotIn("test-protected-key", process.stdout + process.stderr)

    def test_stalled_download_times_out_reports_progress_and_continues(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), StalledFontStub)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                sample = Image.new("L", (500, 260), 255)
                ImageDraw.Draw(sample).text((20, 20), "G", font=ImageFont.truetype(str(SERIF), 180), fill=0)
                sample.save(root / "sample.png")
                process = subprocess.run([
                    sys.executable, str(SCRIPT), str(root / "sample.png"), "--text", "G",
                    "--api-url", f"http://127.0.0.1:{server.server_port}/webfonts/v1/webfonts",
                    "--api-key", "test-key", "--variant", "700", "--limit", "2",
                    "--cache-dir", str(root / "cache"), "--allow-insecure-localhost",
                    "--download-timeout", "0.1", "--progress",
                ], capture_output=True, text=True, check=False, timeout=3)
                self.assertEqual(process.returncode, 0, process.stderr)
                self.assertIn("[1/2] Stalled", process.stderr)
                self.assertIn("[2/2] Working", process.stderr)
                output = json.loads(process.stdout)
                self.assertEqual(output["ranked"][0]["family"], "Working")
                self.assertEqual(output["rejected"][0]["family"], "Stalled")
        finally:
            server.shutdown()
            server.server_close()

    def test_process_ranks_exact_font_and_preserves_rejections_without_key_leak(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), GoogleFontsStub)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                font = ImageFont.truetype(str(SERIF), 180)
                sample = Image.new("L", (1000, 260), 255)
                ImageDraw.Draw(sample).text((20, 20), "Innovation", font=font, fill=0)
                sample.save(root / "sample.png")
                secret = "test-secret-must-not-leak"
                process = subprocess.run([
                    sys.executable, str(SCRIPT), str(root / "sample.png"), "--text", "Innovation",
                    "--api-url", f"http://127.0.0.1:{server.server_port}/webfonts/v1/webfonts",
                    "--api-key", secret, "--variant", "700", "--limit", "3",
                    "--cache-dir", str(root / "cache"), "--allow-insecure-localhost",
                ], capture_output=True, text=True, check=False)
                self.assertEqual(process.returncode, 0, process.stderr)
                self.assertNotIn(secret, process.stdout + process.stderr)
                output = json.loads(process.stdout)
                self.assertEqual(output["ranked"][0]["family"], "Exact Serif")
                self.assertEqual(output["rejected"], [{"family": "Missing Bold", "reason": "variant_missing"}])
                self.assertEqual(GoogleFontsStub.query["category"], ["serif"])
                self.assertEqual(GoogleFontsStub.query["key"], [secret])
        finally:
            server.shutdown()
            server.server_close()

    def test_missing_official_key_fails_before_network(self):
        with tempfile.TemporaryDirectory() as directory:
            process = subprocess.run([
                sys.executable, str(SCRIPT), "/does/not/matter.png", "--text", "Innovation",
            ], capture_output=True, text=True, check=False,
               env={"HOME": directory, "PATH": os.environ.get("PATH", "")})
            self.assertEqual(process.returncode, 2)
            self.assertIn("GOOGLE_FONTS_API_KEY", process.stderr)

    def test_network_error_does_not_leak_api_key(self):
        secret = "network-secret-must-not-leak"
        process = subprocess.run([
            sys.executable, str(SCRIPT), "/does/not/matter.png", "--text", "Innovation",
            "--api-url", "http://127.0.0.1:1/webfonts/v1/webfonts", "--api-key", secret,
            "--allow-insecure-localhost",
        ], capture_output=True, text=True, check=False)
        self.assertEqual(process.returncode, 2)
        self.assertNotIn(secret, process.stdout + process.stderr)
        self.assertIn("falha ao consultar catálogo", process.stderr)

    def test_multiple_discriminatory_glyphs_select_family_weight_and_italic_style(self):
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), GoogleFontsStub)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                samples = []
                for name, text, font_path in [("g", "g", SERIF), ("y", "y", SERIF_ITALIC)]:
                    image = Image.new("L", (260, 260), 255)
                    ImageDraw.Draw(image).text((30, 20), text, font=ImageFont.truetype(str(font_path), 180), fill=0)
                    image.save(root / f"{name}.png")
                    samples.append({"id": name, "path": f"{name}.png", "text": text, "weight": 4,
                                    "dimensions": ["family", "weight" if name == "g" else "style"],
                                    "variants": ["700"] if name == "g" else ["italic"]})
                evidence = root / "evidence.json"
                evidence.write_text(json.dumps({"samples": samples}))
                process = subprocess.run([
                    sys.executable, str(SCRIPT), "--evidence", str(evidence),
                    "--api-url", f"http://127.0.0.1:{server.server_port}/webfonts/v1/webfonts",
                    "--api-key", "test-key", "--limit", "3", "--cache-dir", str(root / "cache"),
                    "--allow-insecure-localhost",
                ], capture_output=True, text=True, check=False)
                self.assertEqual(process.returncode, 0, process.stderr)
                output = json.loads(process.stdout)
                winner = output["ranked"][0]
                self.assertEqual(winner["family"], "Exact Serif")
                self.assertEqual([item["variant"] for item in winner["evidence"]], ["700", "italic"])
                self.assertIsNotNone(winner["family_score"])
                self.assertIsNotNone(winner["weight_score"])
                self.assertIsNotNone(winner["style_score"])
                self.assertIn("Missing Bold", [item["family"] for item in output["rejected"]])
        finally:
            server.shutdown()
            server.server_close()


if __name__ == "__main__":
    unittest.main()
