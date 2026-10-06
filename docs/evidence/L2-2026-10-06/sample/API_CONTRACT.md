# Assembled product entrypoint

Use `python3 -B app.py` on 127.0.0.1:8080 for UI and API together. Default persistent file: data/history.sqlite3 under the product root. GET /api/generations is a same-origin alias of GET /api/history with the same response shape. The standalone server.py entrypoint below is retained for backend-only development; it is not the recommended product launcher.

# Local title API

Requires Python 3.10+; standard library only. Run from this directory:

    python3 server.py

Binds only to 127.0.0.1:8000. Stop using Ctrl-C. SQLite history is stored in title_history.sqlite3 relative to the working directory; restart from the same directory to retain it. If port 8000 is occupied, stop the conflicting process or import create_app(repository, port=another_port).

## Generation

POST /api/generate with Content-Type: application/json

    {"product_idea":"Focus timer"}

HTTP 200 after persistence succeeds:

    {"ok":true,"result":{"id":"UUID","product_idea":"Focus timer","titles":["Focus timer made simple","A better way to use Focus timer","Focus timer in minutes"],"created_at":"2026-10-06T00:00:00Z"}}

IDs and UTC timestamps are generated per record. Input is trimmed. Generation is a deterministic local template, not an AI model or external service. No API key or paid resources required.

## History

GET /api/history returns HTTP 200:

    {"ok":true,"history":[]}

Each history entry uses the same result fields. Most recent first; up to 50 records (older records remain in the database).

## Errors

All documented failures use this JSON shape:

    {"ok":false,"error":{"code":"INVALID_INPUT","message":"product_idea is required"}}

- 400 / INVALID_INPUT: malformed JSON, non-object body, missing/blank/non-string product_idea, invalid or negative Content-Length.
- 413 / INVALID_INPUT: body larger than 1,000,000 bytes.
- 404 / NOT_FOUND: unrecognized GET or POST route.
- 503 / STORAGE_ERROR: SQLite read/write failure; message is History storage is unavailable (no internal SQL or filesystem details exposed).

Only GET and POST routes above are supported. This is a loopback-only development server, not a production web server. No static UI or CORS policy is supplied; the frontend task must decide same-origin serving or an explicit local development proxy.

## Tests

    python3 -B -m unittest discover -s tests -v

Tests use temporary SQLite files and ephemeral ports, covering real HTTP generation/history, restart persistence, malformed input, negative/oversized lengths, routing and database failures.
