ALTER TABLE repositories ADD COLUMN forked_from INTEGER REFERENCES repositories(id) ON DELETE SET NULL;
