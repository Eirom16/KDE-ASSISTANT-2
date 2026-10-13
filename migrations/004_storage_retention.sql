-- Additive indexes only. Retention is non-destructive by default; legacy rows unchanged.
CREATE INDEX IF NOT EXISTS idx_turns_session ON conversation_turns(session_id);
CREATE INDEX IF NOT EXISTS idx_audit_session ON tool_audit(session_id);
CREATE INDEX IF NOT EXISTS idx_messages_context ON messages(session_id,role,id);
INSERT OR IGNORE INTO recovery_migrations VALUES(4,strftime('%Y-%m-%dT%H:%M:%fZ','now'));
-- Parent deletion must never hide dependants, even with legacy FK mode.
CREATE TRIGGER IF NOT EXISTS storage_session_delete BEFORE DELETE ON sessions
WHEN EXISTS(SELECT 1 FROM messages WHERE session_id=OLD.id)
 OR EXISTS(SELECT 1 FROM conversation_turns WHERE session_id=OLD.id)
 OR EXISTS(SELECT 1 FROM operation_journal WHERE session_id=OLD.id)
 OR EXISTS(SELECT 1 FROM tool_audit WHERE session_id=OLD.id)
BEGIN SELECT RAISE(ABORT,'session_dependants_exist'); END;
CREATE TRIGGER IF NOT EXISTS storage_turn_delete BEFORE DELETE ON conversation_turns
WHEN EXISTS(SELECT 1 FROM operation_journal WHERE turn_id=OLD.id)
 OR EXISTS(SELECT 1 FROM turn_inputs WHERE turn_id=OLD.id)
 OR EXISTS(SELECT 1 FROM turn_recovery WHERE turn_id=OLD.id)
 OR EXISTS(SELECT 1 FROM messages WHERE turn_id=OLD.id)
BEGIN SELECT RAISE(ABORT,'turn_dependants_exist'); END;
CREATE TRIGGER IF NOT EXISTS storage_message_delete BEFORE DELETE ON messages
WHEN EXISTS(SELECT 1 FROM turn_inputs WHERE user_message_id=OLD.id)
BEGIN SELECT RAISE(ABORT,'message_dependants_exist'); END;
CREATE TRIGGER IF NOT EXISTS storage_operation_delete BEFORE DELETE ON operation_journal
WHEN EXISTS(SELECT 1 FROM operation_events WHERE operation_id=OLD.id)
BEGIN SELECT RAISE(ABORT,'operation_dependants_exist'); END;
CREATE TRIGGER IF NOT EXISTS storage_audit_delete BEFORE DELETE ON tool_audit
WHEN EXISTS(SELECT 1 FROM operation_journal WHERE audit_id=OLD.id)
 OR EXISTS(SELECT 1 FROM operation_events WHERE audit_id=OLD.id)
BEGIN SELECT RAISE(ABORT,'audit_dependants_exist'); END;
CREATE TRIGGER IF NOT EXISTS storage_message_session BEFORE INSERT ON messages
WHEN NOT EXISTS(SELECT 1 FROM sessions WHERE id=NEW.session_id)
BEGIN SELECT RAISE(ABORT,'message_session_missing'); END;
