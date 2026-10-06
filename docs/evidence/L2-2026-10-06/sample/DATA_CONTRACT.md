# Data contract

The prototype stores generated title history in a local SQLite database. The repository is independent of HTTP/UI code.

## Record

- id: UUID string
- product_idea: non-empty trimmed string
- titles: exactly three non-empty strings
- created_at: UTC ISO-8601 timestamp ending in Z

HistoryRepository(database_path) creates the database and title_history table if absent. create(product_idea, titles) validates and persists one record. list_recent(limit=50) returns records newest first. The database path is supplied by the application; no cloud service or secret is required.
