import http.client
import json
import sqlite3
import tempfile
import threading
import unittest
from pathlib import Path
from server import create_app
from title_history import HistoryRepository

class HTTPBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.db = Path(self.temp.name) / 'history.sqlite3'
        self.repo = HistoryRepository(self.db)
        self.app = create_app(self.repo, port=0)
        self.thread = threading.Thread(target=self.app.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.app.shutdown()
        self.app.server_close()
        self.thread.join()
        self.temp.cleanup()

    def request(self, method, route, body=None, headers=None):
        conn = http.client.HTTPConnection('127.0.0.1', self.app.server_port, timeout=2)
        try:
            conn.request(method, route, body, headers or {})
            response = conn.getresponse()
            return response.status, json.loads(response.read())
        finally:
            conn.close()

    def test_invalid_bodies_do_not_persist(self):
        for body in [b'{', b'[]', b'null', b'{}', b'{"product_idea":1}', b'{"product_idea":"  "}', b'\xff']:
            with self.subTest(body=body):
                status, payload = self.request('POST', '/api/generate', body)
                self.assertEqual(status, 400)
                self.assertFalse(payload['ok'])
                self.assertEqual(payload['error']['code'], 'INVALID_INPUT')
        self.assertEqual(self.repo.list_recent(), [])

    def test_negative_length_is_rejected_without_waiting_for_eof(self):
        status, payload = self.request('POST', '/api/generate', headers={'Content-Length':'-1'})
        self.assertEqual(status, 400)
        self.assertEqual(payload['error']['code'], 'INVALID_INPUT')

    def test_oversize_and_unknown_routes(self):
        self.assertEqual(self.request('POST', '/api/generate', headers={'Content-Length':'1000001'})[0], 413)
        status, payload = self.request('GET', '/missing')
        self.assertEqual(status, 404)
        self.assertEqual(payload['error']['code'], 'NOT_FOUND')

    def test_storage_failure_is_json_not_disconnected_socket(self):
        with sqlite3.connect(self.db) as connection:
            connection.execute('DROP TABLE title_history')
        for method, route, body in [('GET','/api/history',None), ('POST','/api/generate',b'{"product_idea":"test"}')]:
            with self.subTest(route=route):
                status, payload = self.request(method, route, body)
                self.assertEqual(status, 503)
                self.assertEqual(payload, {'ok':False,'error':{'code':'STORAGE_ERROR','message':'History storage is unavailable'}})

    def test_record_survives_server_restart(self):
        status, generated = self.request('POST', '/api/generate', json.dumps({'product_idea':' 专注计时器 '}).encode())
        self.assertEqual(status, 200)
        self.assertEqual(generated['result']['product_idea'], '专注计时器')
        self.app.shutdown()
        self.app.server_close()
        self.thread.join()
        self.app = create_app(HistoryRepository(self.db), port=0)
        self.thread = threading.Thread(target=self.app.serve_forever, daemon=True)
        self.thread.start()
        status, history = self.request('GET','/api/history')
        self.assertEqual(status, 200)
        self.assertEqual(history['history'], [generated['result']])
