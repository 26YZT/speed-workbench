from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from title_history import HistoryRepository, ValidationError

class HistoryRepositoryTests(unittest.TestCase):
    def test_creates_and_reads_persistent_history(self):
        with tempfile.TemporaryDirectory() as directory:
            database = Path(directory) / "history.sqlite3"
            repository = HistoryRepository(database)
            record = repository.create("AI meal planner", ["Plan dinners", "Meal magic", "Weekly kitchen"])
            self.assertEqual(record.product_idea, "AI meal planner")
            self.assertEqual(record.titles, ("Plan dinners", "Meal magic", "Weekly kitchen"))
            self.assertTrue(record.id)
            self.assertTrue(record.created_at.endswith("Z"))
            reopened = HistoryRepository(database)
            self.assertEqual(reopened.list_recent(), [record])

    def test_rejects_invalid_contract_values(self):
        with tempfile.TemporaryDirectory() as directory:
            repository = HistoryRepository(Path(directory) / "history.sqlite3")
            for case in [("", ["a", "b", "c"]), ("idea", ["a", "b"]), ("idea", ["a", "b", ""])]:
                with self.subTest(case=case):
                    with self.assertRaises(ValidationError):
                        repository.create(*case)

    def test_json_contract_is_stable_and_ordered(self):
        with tempfile.TemporaryDirectory() as directory:
            record = HistoryRepository(Path(directory) / "history.sqlite3").create("Idea", ["One", "Two", "Three"])
            payload = record.to_dict()
            self.assertEqual(list(payload), ["id", "product_idea", "titles", "created_at"])
            self.assertEqual(json.loads(json.dumps(payload))["titles"], ["One", "Two", "Three"])

if __name__ == "__main__":
    unittest.main()
