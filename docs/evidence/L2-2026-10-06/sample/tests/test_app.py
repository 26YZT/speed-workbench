import json
import tempfile
import threading
import unittest
from pathlib import Path
from urllib.request import Request, urlopen
from app import create_app

class FrontendHostTests(unittest.TestCase):
    def test_assets_api_alias_and_persistence(self):
        with tempfile.TemporaryDirectory() as directory:
            database=Path(directory)/'history.sqlite3'
            saved=None
            for cycle in range(2):
                app=create_app(database, port=0)
                thread=threading.Thread(target=app.serve_forever, daemon=True)
                thread.start()
                base=f'http://127.0.0.1:{app.server_port}'
                try:
                    for asset in ['/', '/app.js', '/style.css']:
                        with urlopen(base+asset) as response:
                            self.assertEqual(response.status,200)
                            self.assertTrue(response.read())
                    if cycle==0:
                        request=Request(base+'/api/generate', data=json.dumps({'product_idea':'专注工具'}).encode(),headers={'Content-Type':'application/json'})
                        with urlopen(request) as response:
                            saved=json.load(response)['result']
                        self.assertEqual(len(saved['titles']),3)
                    with urlopen(base+'/api/generations') as response:
                        self.assertEqual(json.load(response)['history'],[saved])
                finally:
                    app.shutdown(); app.server_close(); thread.join()
