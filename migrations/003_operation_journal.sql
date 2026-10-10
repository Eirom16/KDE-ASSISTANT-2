-- Additive migration; executed inside the startup transaction. Old messages unchanged.
CREATE TABLE IF NOT EXISTS recovery_migrations (
    version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS operation_journal (
    id TEXT PRIMARY KEY NOT NULL,
    turn_id TEXT NOT NULL REFERENCES conversation_turns(id),
    session_id TEXT,
    kind TEXT NOT NULL CHECK(kind='tool'),
    tool TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('prepared','authorized','dispatched','succeeded','failed','denied','unknown_outcome','abandoned')),
    prepared_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    permission TEXT NOT NULL CHECK(permission IN ('green','yellow','red')),
    consent TEXT NOT NULL CHECK(consent IN ('pending','auto','approved','denied')),
    audit_id INTEGER NOT NULL REFERENCES tool_audit(id),
    result_bytes INTEGER CHECK(result_bytes IS NULL OR result_bytes>=0),
    CHECK(state != 'prepared' OR consent='pending'),
    CHECK(state NOT IN ('authorized','dispatched','succeeded','failed','unknown_outcome') OR consent IN ('auto','approved')),
    CHECK(state != 'denied' OR consent='denied')
);
CREATE INDEX IF NOT EXISTS idx_operations_turn ON operation_journal(turn_id);
CREATE INDEX IF NOT EXISTS idx_operations_session ON operation_journal(session_id,prepared_at);
CREATE INDEX IF NOT EXISTS idx_operations_pending ON operation_journal(state)
    WHERE state IN ('prepared','authorized','dispatched');
CREATE TABLE IF NOT EXISTS operation_events (
    id INTEGER PRIMARY KEY,
    operation_id TEXT NOT NULL REFERENCES operation_journal(id),
    state TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    audit_id INTEGER NOT NULL REFERENCES tool_audit(id)
);
CREATE INDEX IF NOT EXISTS idx_operation_events ON operation_events(operation_id,id);
CREATE TABLE IF NOT EXISTS turn_recovery (
    turn_id TEXT PRIMARY KEY REFERENCES conversation_turns(id),
    previous_state TEXT NOT NULL,
    recovered_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS turn_inputs (
    turn_id TEXT PRIMARY KEY REFERENCES conversation_turns(id),
    user_message_id INTEGER NOT NULL REFERENCES messages(id)
);
CREATE INDEX IF NOT EXISTS idx_turn_inputs_message ON turn_inputs(user_message_id);
-- Legacy schema does not enable FK enforcement globally. New records enforce
-- ownership and linkage locally without changing old deletion behavior.
CREATE TRIGGER IF NOT EXISTS operation_owner BEFORE INSERT ON operation_journal
WHEN NOT EXISTS(SELECT 1 FROM conversation_turns WHERE id=NEW.turn_id AND session_id IS NEW.session_id)
 OR NOT EXISTS(SELECT 1 FROM tool_audit WHERE id=NEW.audit_id)
BEGIN SELECT RAISE(ABORT,'invalid_operation_owner'); END;
CREATE TRIGGER IF NOT EXISTS operation_transition BEFORE UPDATE OF state ON operation_journal
WHEN NOT ((OLD.state='prepared' AND NEW.state IN ('authorized','denied','abandoned'))
 OR (OLD.state='authorized' AND NEW.state IN ('dispatched','abandoned'))
 OR (OLD.state='dispatched' AND NEW.state IN ('succeeded','failed','unknown_outcome')))
BEGIN SELECT RAISE(ABORT,'invalid_operation_transition'); END;
CREATE TRIGGER IF NOT EXISTS operation_event_link BEFORE INSERT ON operation_events
WHEN NOT EXISTS(SELECT 1 FROM operation_journal WHERE id=NEW.operation_id AND state=NEW.state AND audit_id=NEW.audit_id)
 OR NOT EXISTS(SELECT 1 FROM tool_audit WHERE id=NEW.audit_id)
BEGIN SELECT RAISE(ABORT,'invalid_operation_event'); END;
INSERT OR IGNORE INTO recovery_migrations VALUES(3,strftime('%Y-%m-%dT%H:%M:%fZ','now'));

CREATE TRIGGER IF NOT EXISTS operation_identity BEFORE UPDATE OF id,turn_id,session_id,kind,tool,prepared_at,permission ON operation_journal
BEGIN SELECT RAISE(ABORT,'operation_identity_immutable'); END;
