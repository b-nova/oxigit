# Fly.io Operations Runbook

## Deployment

```bash
# Deploy from main
fly deploy

# Deploy to staging first (recommended)
fly deploy -a oxigit-staging
# validate, then:
fly deploy -a oxigit
```

Auto-deploy: add `fly deploy` step to `.github/workflows/docker.yml` after image push.

## Backups

### Automatic
Fly.io snapshots volumes daily. Verify:
```bash
fly volumes snapshots list
```

### Manual SQLite backup
```bash
fly ssh console -C "sqlite3 /app/data/oxigit.db '.backup /tmp/oxigit-backup.db'"
fly ssh sftp get /tmp/oxigit-backup.db ./oxigit-backup.db
```

### Full data export (DB + repos)
```bash
fly ssh console -C "tar czf /tmp/oxigit-data.tar.gz -C /app data"
fly ssh sftp get /tmp/oxigit-data.tar.gz ./oxigit-data.tar.gz
```

### Restore
- From volume snapshot: `fly volumes fork <snapshot-id>`
- From backup file: upload via `fly ssh sftp` and replace files

## Monitoring

### Health check
Already configured in `fly.toml`: HTTP GET `/` every 30s.

### Disk usage
```bash
fly ssh console -C "df -h /app/data"
fly ssh console -C "du -sh /app/data/repos /app/data/oxigit.db"
```

### Logs
```bash
fly logs              # live tail
fly logs --app oxigit # specific app
```

### DB integrity
```bash
fly ssh console -C "sqlite3 /app/data/oxigit.db 'PRAGMA integrity_check'"
```

## Incident Response

| Problem | Fix |
|---------|-----|
| Instance stuck | `fly restart` or `fly machine restart` |
| Volume full | `fly volumes extend <vol_id> --size <new_gb>` |
| DB corruption | Restore from latest volume snapshot |
| SSH host key changed | Users must update `~/.ssh/known_hosts` |
| App won't start | Check `fly logs`, fix config, `fly deploy` |

## Secrets Management

```bash
fly secrets list                           # view configured secrets
fly secrets set OXIGIT_SECRET_KEY=<hex>    # session signing key
fly secrets set OXIGIT_LLM_PROVIDER=anthropic OXIGIT_LLM_API_KEY=<key>
fly secrets set STRIPE_SECRET_KEY=<key> STRIPE_WEBHOOK_SECRET=<key>
```

## Bootstrap First Admin

After initial deploy with the admin migration:
```bash
fly ssh console -C "sqlite3 /app/data/oxigit.db \"UPDATE users SET is_admin=1 WHERE username='your_username'\""
```

Then manage other admins from the `/admin` dashboard.

## Scaling Notes

- SQLite is single-writer. Do not scale to multiple Fly instances without migrating to PostgreSQL.
- Current pool: `max_connections(5)` — sufficient for moderate load.
- Signs of bottleneck: write contention errors, slow response on push/repo creation.
- Migration path: PostgreSQL via `sqlx` — most queries are standard SQL with minor SQLite-specific syntax (datetime functions).
