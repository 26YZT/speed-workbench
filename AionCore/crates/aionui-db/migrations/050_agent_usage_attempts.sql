-- Preserve all history while separating each actual Send in a logical turn.
CREATE TABLE IF NOT EXISTS agent_usage_attempts_new (
 id TEXT PRIMARY KEY NOT NULL, user_id TEXT NOT NULL, task_id TEXT, agent_id TEXT, model TEXT,
 input_tokens INTEGER NOT NULL DEFAULT 0 CHECK(input_tokens>=0),
 output_tokens INTEGER NOT NULL DEFAULT 0 CHECK(output_tokens>=0), cost_est REAL,
 conversation_id TEXT NOT NULL, turn_id TEXT NOT NULL, created_at INTEGER NOT NULL,
 cached_read_tokens INTEGER CHECK(cached_read_tokens>=0), cached_write_tokens INTEGER CHECK(cached_write_tokens>=0),
 cost_source TEXT, pricing_snapshot TEXT, cost_unknown_reason TEXT,
 team_id TEXT, attempt_id TEXT NOT NULL DEFAULT 'legacy', UNIQUE(user_id,conversation_id,turn_id,attempt_id)
);
INSERT INTO agent_usage_attempts_new (id,user_id,task_id,agent_id,model,input_tokens,output_tokens,cost_est,conversation_id,turn_id,created_at,cached_read_tokens,cached_write_tokens,cost_source,pricing_snapshot,cost_unknown_reason)
 SELECT id,user_id,task_id,agent_id,model,input_tokens,output_tokens,cost_est,conversation_id,turn_id,created_at,cached_read_tokens,cached_write_tokens,cost_source,pricing_snapshot,cost_unknown_reason FROM agent_usage;
DROP TABLE agent_usage;
ALTER TABLE agent_usage_attempts_new RENAME TO agent_usage;
CREATE INDEX IF NOT EXISTS idx_agent_usage_user_task_created ON agent_usage(user_id,task_id,created_at DESC,id DESC);
CREATE INDEX IF NOT EXISTS idx_agent_usage_user_conversation_created ON agent_usage(user_id,conversation_id,created_at DESC,id DESC);

UPDATE agent_usage SET team_id=(SELECT teams.id FROM team_tasks JOIN teams ON teams.id=team_tasks.team_id WHERE team_tasks.id=agent_usage.task_id AND teams.user_id=agent_usage.user_id LIMIT 1) WHERE task_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_agent_usage_user_team_created ON agent_usage(user_id,team_id,created_at DESC,id DESC);
