import tempfile
import json
import threading
import unittest
from urllib.request import Request, urlopen
from pathlib import Path

from server import create_app, handle_generate, handle_history
from title_history import HistoryRepository

class BackendContractTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.repo = HistoryRepository(Path(self.temp.name) / "history.sqlite3")

    def tearDown(self):
        self.temp.cleanup()

    def test_generate_returns_three_titles_and_persists_record(self):
        response = handle_generate({"product_idea": "Focus timer"}, self.repo)
        self.assertEqual(response["ok"], True)
        self.assertIn("result", response)
        result = response["result"]
        self.assertEqual(result["product_idea"], "Focus timer")
        self.assertEqual(len(result["titles"]), 3)
        self.assertEqual(len(handle_history(self.repo)["history"]), 1)

    def test_invalid_payload_returns_stable_error_without_persisting(self):
        response = handle_generate({}, self.repo)
        self.assertEqual(response, {"ok": False, "error": {"code": "INVALID_INPUT", "message": "product_idea is required"}})
        self.assertEqual(handle_history(self.repo), {"ok": True, "history": []})

    def test_http_app_is_constructible(self):
        app = create_app(self.repo, port=0)
        try:
            self.assertTrue(hasattr(app, "serve_forever"))
            self.assertNotEqual(app.server_port, 0)
        finally:
            app.server_close()

    def test_http_generate_and_history_routes(self):
        app = create_app(self.repo, port=0)
        thread = threading.Thread(target=app.serve_forever, daemon=True)
        thread.start()
        base = f"http://127.0.0.1:{app.server_port}"
        try:
            request = Request(base + "/api/generate", data=json.dumps({"product_idea": "Inbox zero"}).encode(), headers={"Content-Type": "application/json"}, method="POST")
            with urlopen(request) as response:
                generated = json.load(response)
            self.assertTrue(generated["ok"])
            with urlopen(base + "/api/history") as response:
                history = json.load(response)
            self.assertEqual(len(history["history"]), 1)
            self.assertEqual(history["history"][0]["product_idea"], "Inbox zero")
        finally:
            app.shutdown()
            app.server_close()

if __name__ == "__main__":
    unittest.main()
