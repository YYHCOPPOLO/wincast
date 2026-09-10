//! Local `ai-chats.sqlite3`. Empty chats are never saved. Off does not open the file.

use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OptionalExtension};
use tinycast_pure::ai::{
    ChatConversation, ChatMessage, ChatRole, ChatSession, ChatState, Uuid,
};

const SCHEMA: &str = "
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS conversations(
  id TEXT PRIMARY KEY NOT NULL,
  title TEXT NOT NULL,
  preview TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  message_count INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS messages(
  id TEXT PRIMARY KEY NOT NULL,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  role TEXT NOT NULL,
  text TEXT NOT NULL,
  state TEXT NOT NULL,
  sent_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS messages_by_conversation
  ON messages(conversation_id, position);
CREATE INDEX IF NOT EXISTS conversations_by_recency
  ON conversations(updated_at DESC);
";

pub struct ChatHistoryStore {
    dir: PathBuf,
    conn: Option<Connection>,
    conversations: Vec<ChatConversation>,
}

impl ChatHistoryStore {
    pub fn closed(dir: PathBuf) -> Self {
        Self {
            dir,
            conn: None,
            conversations: Vec::new(),
        }
    }

    pub fn in_roaming() -> Self {
        Self::closed(crate::platform::paths::roaming_dir())
    }

    pub fn conversations(&self) -> &[ChatConversation] {
        &self.conversations
    }

    pub fn is_open(&self) -> bool {
        self.conn.is_some()
    }

    pub fn load(&mut self) {
        if self.ensure().is_none() {
            return;
        }
        self.reload();
    }

    pub fn close(&mut self) {
        self.conn = None;
        self.conversations.clear();
    }

    pub fn search(&self, query: &str) -> Vec<ChatConversation> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.conversations.clone();
        }
        self.conversations
            .iter()
            .filter(|c| {
                c.title.to_lowercase().contains(&q) || c.preview.to_lowercase().contains(&q)
            })
            .cloned()
            .collect()
    }

    pub fn session(&self, id: &Uuid) -> Option<ChatSession> {
        let conn = self.conn.as_ref()?;
        let (created_at, updated_at): (i64, i64) = conn
            .query_row(
                "SELECT created_at, updated_at FROM conversations WHERE id = ?1",
                params![id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .ok()
            .flatten()?;
        let mut stmt = conn
            .prepare(
                "SELECT id, role, text, state, sent_at FROM messages
                 WHERE conversation_id = ?1 ORDER BY position",
            )
            .ok()?;
        let rows = stmt
            .query_map(params![id.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .ok()?;
        let mut messages = Vec::new();
        for row in rows.flatten() {
            let (mid, role, mut text, state, sent_at) = row;
            let Some(role) = ChatRole::parse(&role) else {
                continue;
            };
            let stored = ChatState::parse(&state).unwrap_or(ChatState::Complete);
            let state = if stored == ChatState::Streaming {
                if text.is_empty() {
                    text = "Response interrupted.".into();
                }
                ChatState::Failed
            } else {
                stored
            };
            messages.push(ChatMessage {
                id: Uuid::parse(mid),
                role,
                text,
                state,
                sent_at,
            });
        }
        Some(ChatSession {
            id: id.clone(),
            created_at,
            updated_at,
            messages,
        })
    }

    pub fn save(&mut self, session: &ChatSession) {
        if session.messages.is_empty() {
            return;
        }
        if self.ensure().is_none() {
            return;
        }
        let summary = session.summary();
        {
            let Some(conn) = self.conn.as_mut() else {
                return;
            };
            let Ok(tx) = conn.transaction() else {
                return;
            };
            // Every early return drops the transaction, rolling back the previous
            // conversation, its messages, and any failed statement's side effects.
            if tx.execute(
                "INSERT INTO conversations(id, title, preview, created_at, updated_at, message_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(id) DO UPDATE SET
                   title=excluded.title, preview=excluded.preview,
                   updated_at=excluded.updated_at, message_count=excluded.message_count",
                params![
                    summary.id.as_str(),
                    summary.title,
                    summary.preview,
                    summary.created_at,
                    summary.updated_at,
                    summary.message_count as i64
                ],
            ).is_err() {
                return;
            }
            if tx.execute(
                "DELETE FROM messages WHERE conversation_id = ?1",
                params![session.id.as_str()],
            ).is_err() {
                return;
            }
            for (i, message) in session.messages.iter().enumerate() {
                if tx.execute(
                    "INSERT INTO messages(id, conversation_id, position, role, text, state, sent_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        message.id.as_str(),
                        session.id.as_str(),
                        i as i64,
                        message.role.as_str(),
                        message.text,
                        message.state.as_str(),
                        message.sent_at
                    ],
                ).is_err() {
                    return;
                }
            }
            if tx.commit().is_err() {
                return;
            }
        }
        self.reload();
    }

    pub fn delete(&mut self, id: &Uuid) {
        if let Some(conn) = self.conn.as_mut() {
            let _ = conn.execute(
                "DELETE FROM conversations WHERE id = ?1",
                params![id.as_str()],
            );
        }
        self.reload();
    }

    pub fn prune_before(&mut self, cutoff: i64) {
        let changed = if let Some(conn) = self.conn.as_mut() {
            conn.execute(
                "DELETE FROM conversations WHERE updated_at < ?1",
                params![cutoff],
            )
            .unwrap_or(0)
        } else {
            return;
        };
        if changed > 0 {
            if let Some(conn) = self.conn.as_mut() {
                let _ = conn.execute_batch("VACUUM");
            }
        }
        self.reload();
    }

    fn ensure(&mut self) -> Option<&Connection> {
        if self.conn.is_none() {
            let _ = std::fs::create_dir_all(&self.dir);
            let path = self.dir.join("ai-chats.sqlite3");
            let conn = Connection::open(&path).ok()?;
            conn.execute_batch(SCHEMA).ok()?;
            self.conn = Some(conn);
        }
        self.conn.as_ref()
    }

    fn reload(&mut self) {
        let Some(conn) = self.conn.as_ref() else {
            self.conversations.clear();
            return;
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT id, title, preview, created_at, updated_at, message_count
             FROM conversations ORDER BY updated_at DESC",
        ) else {
            return;
        };
        let rows = stmt.query_map([], |row| {
            Ok(ChatConversation {
                id: Uuid::parse(row.get::<_, String>(0)?),
                title: row.get(1)?,
                preview: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
                message_count: row.get::<_, i64>(5)? as usize,
            })
        });
        let Ok(rows) = rows else {
            return;
        };
        self.conversations = rows.flatten().collect();
    }
}

pub fn db_path(dir: &Path) -> PathBuf {
    dir.join("ai-chats.sqlite3")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tinycast_pure::ai::ChatMessage;

    fn temp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "tinycast-ai-chats-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn memory_store() -> ChatHistoryStore {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        ChatHistoryStore {
            dir: PathBuf::new(),
            conn: Some(conn),
            conversations: Vec::new(),
        }
    }

    fn saved_conversation() -> (ChatHistoryStore, ChatSession) {
        let mut store = memory_store();
        let mut session = ChatSession::new(1);
        session.append(ChatMessage::user("original question", 2));
        let mut answer = ChatMessage::assistant_streaming(3);
        answer.text = "original answer".into();
        answer.state = ChatState::Complete;
        session.append(answer);
        store.save(&session);
        assert_saved(&mut store, &session);
        (store, session)
    }

    fn replacement(original: &ChatSession) -> ChatSession {
        let mut session = original.clone();
        session.messages.clear();
        session.append(ChatMessage::user("replacement question", 10));
        let mut answer = ChatMessage::assistant_streaming(11);
        answer.text = "replacement answer".into();
        answer.state = ChatState::Failed;
        session.append(answer);
        session.append(ChatMessage::user("replacement follow-up", 12));
        session
    }

    fn assert_saved(store: &mut ChatHistoryStore, expected: &ChatSession) {
        assert_eq!(store.conversations(), &[expected.summary()]);
        assert_eq!(store.session(&expected.id), Some(expected.clone()));
        store.reload();
        assert_eq!(store.conversations(), &[expected.summary()]);
        let conn = store.conn.as_ref().unwrap();
        assert!(conn.is_autocommit(), "save must leave no transaction open");
        let count: i64 = conn.query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0)).unwrap();
        assert_eq!(count, expected.messages.len() as i64);
    }

    fn inject_failure(store: &ChatHistoryStore, event: &str) {
        store.conn.as_ref().unwrap().execute_batch(&format!("
            CREATE TABLE attempted_writes(value INTEGER);
            CREATE TRIGGER reject_write {event} BEGIN
                INSERT INTO attempted_writes VALUES(1);
                SELECT RAISE(FAIL, 'injected history failure');
            END;
        ")).unwrap();
    }

    fn assert_trigger_rolled_back(store: &ChatHistoryStore) {
        let attempts: i64 = store.conn.as_ref().unwrap().query_row(
            "SELECT COUNT(*) FROM attempted_writes", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(attempts, 0, "failed statement side effects must roll back too");
    }

    #[test]
    fn failed_message_insert_preserves_complete_conversation_and_can_retry() {
        let (mut store, original) = saved_conversation();
        let updated = replacement(&original);
        inject_failure(&store, "BEFORE INSERT ON messages WHEN NEW.position = 1");
        store.save(&updated);
        assert_saved(&mut store, &original);
        assert_trigger_rolled_back(&store);

        store.conn.as_ref().unwrap().execute_batch("DROP TRIGGER reject_write").unwrap();
        store.save(&updated);
        assert_saved(&mut store, &updated);
    }

    #[test]
    fn duplicate_message_id_preserves_complete_conversation() {
        let (mut store, original) = saved_conversation();
        let mut updated = replacement(&original);
        updated.messages[1].id = updated.messages[0].id.clone();
        store.save(&updated);
        assert_saved(&mut store, &original);
    }

    #[test]
    fn failed_conversation_update_preserves_summary_and_messages() {
        let (mut store, original) = saved_conversation();
        inject_failure(&store, "BEFORE UPDATE ON conversations");
        store.save(&replacement(&original));
        assert_saved(&mut store, &original);
        assert_trigger_rolled_back(&store);
    }

    #[test]
    fn failed_conversation_insert_leaves_no_partial_chat_or_side_effects() {
        let (mut store, original) = saved_conversation();
        inject_failure(&store, "BEFORE INSERT ON conversations");
        let mut added = ChatSession::new(20);
        added.append(ChatMessage::user("must not be saved", 21));
        store.save(&added);
        assert_saved(&mut store, &original);
        assert!(store.session(&added.id).is_none());
        assert_trigger_rolled_back(&store);
    }

    #[test]
    fn failed_message_delete_preserves_summary_and_all_old_messages() {
        let (mut store, original) = saved_conversation();
        inject_failure(&store, "BEFORE DELETE ON messages WHEN OLD.position = 1");
        store.save(&replacement(&original));
        assert_saved(&mut store, &original);
        assert_trigger_rolled_back(&store);
    }

    #[test]
    fn failed_commit_preserves_complete_conversation() {
        let (mut store, original) = saved_conversation();
        store.conn.as_ref().unwrap().execute_batch("
            CREATE TABLE parent(id INTEGER PRIMARY KEY);
            CREATE TABLE child(id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);
            CREATE TRIGGER reject_commit AFTER INSERT ON messages WHEN NEW.position = 1 BEGIN
                INSERT INTO child VALUES(1);
            END;
        ").unwrap();
        store.save(&replacement(&original));
        assert_saved(&mut store, &original);
        let count: i64 = store.conn.as_ref().unwrap().query_row(
            "SELECT COUNT(*) FROM child", [], |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn successful_save_replaces_every_message_and_summary() {
        let (mut store, original) = saved_conversation();
        let mut updated = replacement(&original);
        store.save(&updated);
        assert_saved(&mut store, &updated);

        updated.messages.truncate(1);
        updated.updated_at = 30;
        store.save(&updated);
        assert_saved(&mut store, &updated);
    }

    #[test]
    fn empty_save_keeps_closed_store_lazy() {
        let dir = temp();
        let mut store = ChatHistoryStore::closed(dir.clone());
        assert!(!store.is_open());
        store.save(&ChatSession::new(1));
        assert!(!store.is_open());
        assert!(!dir.exists());
    }

    #[test]
    fn empty_chats_are_not_saved() {
        let dir = temp();
        let mut store = ChatHistoryStore::closed(dir.clone());
        store.load();
        let session = ChatSession::new(1);
        store.save(&session);
        assert!(store.conversations().is_empty());
        let mut filled = ChatSession::new(1);
        filled.append(ChatMessage::user("hello", 1));
        store.save(&filled);
        assert_eq!(store.conversations().len(), 1);
        store.close();
        assert!(!store.is_open());
        assert!(store.conversations().is_empty());
        assert!(dir.join("ai-chats.sqlite3").is_file());
        let _ = std::fs::remove_dir_all(dir);
    }
}
