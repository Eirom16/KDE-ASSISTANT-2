CREATE TABLE IF NOT EXISTS permission_requests (
    operation_id TEXT PRIMARY KEY REFERENCES operation_journal(id),
    consent_id TEXT UNIQUE,
    tool_call_id TEXT NOT NULL,
    fingerprint TEXT NOT NULL,
    risk TEXT NOT NULL CHECK(risk IN ('green','yellow','red')),
    principal TEXT NOT NULL CHECK(principal='local_authenticated'),
    origin TEXT NOT NULL CHECK(origin IN ('chat','voice','manual')),
    state TEXT NOT NULL CHECK(state IN ('auto','pending','approved','denied','consumed','revoked')),
    created_at INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    answered_at INTEGER,
    consumed_at INTEGER,
    CHECK((risk='green' AND state IN ('auto','consumed') AND consent_id IS NULL)
       OR (risk!='green' AND consent_id IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS idx_permission_pending ON permission_requests(state,expires_at);
INSERT OR IGNORE INTO recovery_migrations VALUES(5,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
