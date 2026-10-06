"""Local HTTP API for title generation and persistent history."""
from __future__ import annotations

import json
import sqlite3
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any

from title_history import HistoryRepository, ValidationError


def _error(code: str, message: str) -> dict[str, object]:
    return {"ok": False, "error": {"code": code, "message": message}}


def _titles_for(idea: str) -> list[str]:
    return [f"{idea} made simple", f"A better way to use {idea}", f"{idea} in minutes"]


def handle_generate(payload: Any, repository: HistoryRepository) -> dict[str, object]:
    if not isinstance(payload, dict):
        return _error("INVALID_INPUT", "request body must be a JSON object")
    product_idea = payload.get("product_idea")
    if not isinstance(product_idea, str) or not product_idea.strip():
        return _error("INVALID_INPUT", "product_idea is required")
    try:
        record = repository.create(product_idea, _titles_for(product_idea.strip()))
    except ValidationError as exc:
        return _error("INVALID_INPUT", str(exc))
    return {"ok": True, "result": record.to_dict()}


def handle_history(repository: HistoryRepository) -> dict[str, object]:
    return {"ok": True, "history": [record.to_dict() for record in repository.list_recent()]}


class _Handler(BaseHTTPRequestHandler):
    repository: HistoryRepository

    def _write(self, status: HTTPStatus, payload: dict[str, object]) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path != "/api/history":
            self._write(HTTPStatus.NOT_FOUND, _error("NOT_FOUND", "route not found"))
            return
        try:
            response = handle_history(self.repository)
        except sqlite3.Error:
            self._storage_error()
            return
        self._write(HTTPStatus.OK, response)

    def do_POST(self) -> None:  # noqa: N802
        if self.path != "/api/generate":
            self._write(HTTPStatus.NOT_FOUND, _error("NOT_FOUND", "route not found"))
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length < 0:
                raise ValueError("negative body length")
            if length > 1_000_000:
                self._write(HTTPStatus.REQUEST_ENTITY_TOO_LARGE, _error("INVALID_INPUT", "request body is too large"))
                return
            payload = json.loads(self.rfile.read(length))
        except (ValueError, json.JSONDecodeError):
            self._write(HTTPStatus.BAD_REQUEST, _error("INVALID_INPUT", "request body must be valid JSON"))
            return
        try:
            response = handle_generate(payload, self.repository)
        except sqlite3.Error:
            self._storage_error()
            return
        self._write(HTTPStatus.OK if response["ok"] else HTTPStatus.BAD_REQUEST, response)

    def _storage_error(self) -> None:
        self._write(HTTPStatus.SERVICE_UNAVAILABLE, _error("STORAGE_ERROR", "History storage is unavailable"))

    def log_message(self, format: str, *args: object) -> None:
        return


def create_app(repository: HistoryRepository, host: str = "127.0.0.1", port: int = 8000) -> ThreadingHTTPServer:
    handler = type("TitleToolHandler", (_Handler,), {"repository": repository})
    return ThreadingHTTPServer((host, port), handler)


if __name__ == "__main__":
    app = create_app(HistoryRepository("title_history.sqlite3"))
    print("Title tool API listening on http://127.0.0.1:8000")
    app.serve_forever()
