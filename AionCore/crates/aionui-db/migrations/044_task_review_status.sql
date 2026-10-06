-- Add the human-review gate to persisted task status values.
-- SQLite cannot alter a CHECK constraint in place, so rebuild team_tasks and
-- restore the index and parent-integrity triggers attached to the old table.

CREATE TABLE _team_tasks_review_new (
    id          TEXT    PRIMARY KEY NOT NULL,
    team_id     TEXT    NOT NULL,
    subject     TEXT    NOT NULL,
    description TEXT,
    status      TEXT    NOT NULL DEFAULT 'pending'
                        CHECK (status IN ('pending', 'in_progress', 'in_review', 'completed', 'deleted')),
    owner       TEXT,
    blocked_by  TEXT    NOT NULL DEFAULT '[]',
    blocks      TEXT    NOT NULL DEFAULT '[]',
    metadata    TEXT,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

INSERT INTO _team_tasks_review_new
    (id, team_id, subject, description, status, owner, blocked_by, blocks, metadata, created_at, updated_at)
SELECT
    id, team_id, subject, description, status, owner, blocked_by, blocks, metadata, created_at, updated_at
FROM team_tasks;

ALTER TABLE team_tasks RENAME TO _team_tasks_review_old;
ALTER TABLE _team_tasks_review_new RENAME TO team_tasks;
DROP TABLE _team_tasks_review_old;

CREATE INDEX idx_team_tasks_team_id ON team_tasks(team_id);

CREATE TRIGGER trg_team_tasks_team_parent_insert
BEFORE INSERT ON team_tasks
FOR EACH ROW
WHEN NOT EXISTS (SELECT 1 FROM teams WHERE id = NEW.team_id)
BEGIN
    SELECT RAISE(ABORT, 'team_tasks.team_id must reference teams.id');
END;

CREATE TRIGGER trg_team_tasks_team_parent_update
BEFORE UPDATE OF team_id ON team_tasks
FOR EACH ROW
WHEN NOT EXISTS (SELECT 1 FROM teams WHERE id = NEW.team_id)
BEGIN
    SELECT RAISE(ABORT, 'team_tasks.team_id must reference teams.id');
END;
