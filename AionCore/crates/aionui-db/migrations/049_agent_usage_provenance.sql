-- NULL cache buckets distinguish historical records from a measured zero.
ALTER TABLE agent_usage ADD COLUMN cached_read_tokens INTEGER CHECK (cached_read_tokens >= 0);
ALTER TABLE agent_usage ADD COLUMN cached_write_tokens INTEGER CHECK (cached_write_tokens >= 0);
ALTER TABLE agent_usage ADD COLUMN cost_source TEXT;
ALTER TABLE agent_usage ADD COLUMN pricing_snapshot TEXT;
ALTER TABLE agent_usage ADD COLUMN cost_unknown_reason TEXT;
UPDATE agent_usage SET cost_source = 'legacy_unspecified' WHERE cost_est IS NOT NULL;
UPDATE agent_usage SET cost_unknown_reason = 'legacy_cache_breakdown_missing' WHERE cost_est IS NULL;
