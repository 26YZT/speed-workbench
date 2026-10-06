"""Same-origin frontend host; reuse approved backend without modifying it."""
import argparse
from pathlib import Path
from http.server import ThreadingHTTPServer

ROOT = Path(__file__).resolve().parent
from server import _Handler
from title_history import HistoryRepository

class FrontendHandler(_Handler):
    def do_GET(self):
        if self.path == '/api/generations':
            self.path = '/api/history'
            return super().do_GET()
        assets = {'/': ('index.html','text/html'), '/app.js': ('app.js','text/javascript'), '/style.css': ('style.css','text/css')}
        if self.path not in assets:
            return super().do_GET()
        name, content_type = assets[self.path]
        body = (ROOT / 'static' / name).read_bytes()
        self.send_response(200)
        self.send_header('Content-Type', content_type + '; charset=utf-8')
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-store')
        self.end_headers()
        self.wfile.write(body)

def create_app(database, port=8080):
    handler = type('AppHandler', (FrontendHandler,), {'repository': HistoryRepository(database)})
    return ThreadingHTTPServer(('127.0.0.1', port), handler)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=8080)
    parser.add_argument('--database', type=Path, default=ROOT / 'data' / 'history.sqlite3')
    args = parser.parse_args()
    app = create_app(args.database, args.port)
    print(f'Open http://127.0.0.1:{app.server_port}', flush=True)
    try:
        app.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        app.server_close()
