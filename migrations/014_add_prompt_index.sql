ALTER TABLE ai_commit_metadata ADD COLUMN ai_prompt_index INTEGER;
CREATE INDEX IF NOT EXISTS idx_ai_metadata_prompt
    ON ai_commit_metadata(repo_id, ai_session_id, ai_prompt_index);
