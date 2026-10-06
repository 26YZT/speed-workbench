"""Persistent data contract for generated product-title history."""
from __future__ import annotations

import json
import sqlite3
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable


class ValidationError(ValueError):
    """Raised when a history record violates the public data contract."""


@dataclass(frozen=True)
class HistoryRecord:
    id: str
    product_idea: str
    titles: tuple[str, str, str]
    created_at: str

    def to_dict(self) -> dict[str, object]:
        return {"id": self.id, "product_idea": self.product_idea, "titles": list(self.titles), "created_at": self.created_at}


def _validate(product_idea: str, titles: Iterable[str]) -> tuple[str, tuple[str, str, str]]:
    if not isinstance(product_idea, str) or not product_idea.strip():
        raise ValidationError("product_idea must be a non-empty string")
    normalized_idea = product_idea.strip()
    title_values = tuple(titles)
    if len(title_values) != 3 or any(not isinstance(title, str) or not title.strip() for title in title_values):
        raise ValidationError("titles must contain exactly three non-empty strings")
    return normalized_idea, tuple(title.strip() for title in title_values)


class HistoryRepository:
    """SQLite-backed repository with lazy schema creation."""

    def __init__(self, database_path: str | Path):
        self.database_path = Path(database_path)
        self.database_path.parent.mkdir(parents=True, exist_ok=True)
        with self._connect() as connection:
            connection.execute("""
                CREATE TABLE IF NOT EXISTS title_history (
                    id TEXT PRIMARY KEY,
                    product_idea TEXT NOT NULL,
                    titles_json TEXT NOT NULL,
                    created_at TEXT NOT NULL
                )
            """)

    def _connect(self) -> sqlite3.Connection:
        connection = sqlite3.connect(self.database_path)
        connection.row_factory = sqlite3.Row
        return connection

    def create(self, product_idea: str, titles: Iterable[str]) -> HistoryRecord:
        normalized_idea, normalized_titles = _validate(product_idea, titles)
        record = HistoryRecord(str(uuid.uuid4()), normalized_idea, normalized_titles, datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"))
        with self._connect() as connection:
            connection.execute("INSERT INTO title_history (id, product_idea, titles_json, created_at) VALUES (?, ?, ?, ?)", (record.id, record.product_idea, json.dumps(list(record.titles), ensure_ascii=False), record.created_at))
        return record

    def list_recent(self, limit: int = 50) -> list[HistoryRecord]:
        if not isinstance(limit, int) or isinstance(limit, bool) or limit < 1:
            raise ValidationError("limit must be a positive integer")
        with self._connect() as connection:
            rows = connection.execute("SELECT id, product_idea, titles_json, created_at FROM title_history ORDER BY created_at DESC, rowid DESC LIMIT ?", (limit,)).fetchall()
        return [HistoryRecord(row["id"], row["product_idea"], tuple(json.loads(row["titles_json"])), row["created_at"]) for row in rows]
