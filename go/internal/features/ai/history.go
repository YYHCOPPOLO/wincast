package ai

import (
	"context"
	"database/sql"
	"errors"
	"path/filepath"
	"strings"

	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

const historySchema = `
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
`

// History is the local ai-chats.sqlite3 store. The database stays closed until
// AI is enabled, and an empty session is never written.
type History struct {
	path          string
	db            *sql.DB
	conversations []domainai.Conversation
}

func NewHistory(settingsDir string) *History {
	return &History{path: filepath.Join(settingsDir, "ai-chats.sqlite3")}
}

func (h *History) Open(ctx context.Context) error {
	if h.db != nil {
		return h.Reload(ctx)
	}
	db, err := storage.OpenSQLite(ctx, h.path, false)
	if err != nil {
		return err
	}
	if _, err := db.ExecContext(ctx, historySchema); err != nil {
		db.Close()
		return err
	}
	h.db = db
	return h.Reload(ctx)
}

func (h *History) Close() {
	if h.db != nil {
		_ = h.db.Close()
		h.db = nil
	}
	h.conversations = nil
}

func (h *History) Available() bool { return h.db != nil }

func (h *History) Conversations() []domainai.Conversation {
	return h.conversations
}

func (h *History) Search(query string) []domainai.Conversation {
	needle := strings.ToLower(strings.TrimSpace(query))
	if needle == "" {
		return append([]domainai.Conversation(nil), h.conversations...)
	}
	items := make([]domainai.Conversation, 0, len(h.conversations))
	for _, conversation := range h.conversations {
		if strings.Contains(strings.ToLower(conversation.Title), needle) ||
			strings.Contains(strings.ToLower(conversation.Preview), needle) {
			items = append(items, conversation)
		}
	}
	return items
}

func (h *History) Reload(ctx context.Context) error {
	if h.db == nil {
		h.conversations = nil
		return nil
	}
	rows, err := h.db.QueryContext(ctx,
		`SELECT id, title, preview, created_at, updated_at, message_count
		 FROM conversations ORDER BY updated_at DESC`)
	if err != nil {
		return err
	}
	defer rows.Close()
	var items []domainai.Conversation
	for rows.Next() {
		var item domainai.Conversation
		if err := rows.Scan(&item.ID, &item.Title, &item.Preview, &item.CreatedAt, &item.UpdatedAt, &item.MessageCount); err != nil {
			return err
		}
		items = append(items, item)
	}
	if err := rows.Err(); err != nil {
		return err
	}
	h.conversations = items
	return nil
}

func (h *History) Session(ctx context.Context, id string) (domainai.Session, bool) {
	if h.db == nil {
		return domainai.Session{}, false
	}
	var session domainai.Session
	err := h.db.QueryRowContext(ctx,
		`SELECT created_at, updated_at FROM conversations WHERE id = ?`, id).
		Scan(&session.CreatedAt, &session.UpdatedAt)
	if err != nil {
		return domainai.Session{}, false
	}
	session.ID = id
	rows, err := h.db.QueryContext(ctx,
		`SELECT id, role, text, state, sent_at FROM messages
		 WHERE conversation_id = ? ORDER BY position`, id)
	if err != nil {
		return domainai.Session{}, false
	}
	defer rows.Close()
	for rows.Next() {
		var (
			messageID string
			roleRaw   string
			text      string
			stateRaw  string
			sentAt    int64
		)
		if err := rows.Scan(&messageID, &roleRaw, &text, &stateRaw, &sentAt); err != nil {
			return domainai.Session{}, false
		}
		role, ok := domainai.ParseChatRole(roleRaw)
		if !ok {
			continue
		}
		state, ok := domainai.ParseChatState(stateRaw)
		if !ok {
			state = domainai.StateComplete
		}
		if state == domainai.StateStreaming {
			if text == "" {
				text = "Response interrupted."
			}
			state = domainai.StateFailed
		}
		session.Messages = append(session.Messages,
			domainai.NewChatMessage(messageID, role, text, state, sentAt))
	}
	if err := rows.Err(); err != nil {
		return domainai.Session{}, false
	}
	return session, true
}

// Save replaces one conversation in a single transaction. A failure rolls
// back all statements, keeping the previous conversation intact.
func (h *History) Save(ctx context.Context, session domainai.Session) error {
	if len(session.Messages) == 0 {
		return nil
	}
	if h.db == nil {
		return errors.New("chat history is closed")
	}
	summary := session.Summary()
	tx, err := h.db.BeginTx(ctx, nil)
	if err != nil {
		return err
	}
	defer tx.Rollback()
	if _, err := tx.ExecContext(ctx,
		`INSERT INTO conversations(id, title, preview, created_at, updated_at, message_count)
		 VALUES (?, ?, ?, ?, ?, ?)
		 ON CONFLICT(id) DO UPDATE SET
		   title=excluded.title, preview=excluded.preview,
		   updated_at=excluded.updated_at, message_count=excluded.message_count`,
		summary.ID, summary.Title, summary.Preview, summary.CreatedAt, summary.UpdatedAt, summary.MessageCount); err != nil {
		return err
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM messages WHERE conversation_id = ?`, session.ID); err != nil {
		return err
	}
	statement, err := tx.PrepareContext(ctx,
		`INSERT INTO messages(id, conversation_id, position, role, text, state, sent_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`)
	if err != nil {
		return err
	}
	defer statement.Close()
	for position, message := range session.Messages {
		if _, err := statement.ExecContext(ctx,
			message.ID, session.ID, position, string(message.Role), message.Text,
			string(message.State), message.SentAt); err != nil {
			return err
		}
	}
	if err := tx.Commit(); err != nil {
		return err
	}
	return h.Reload(ctx)
}

func (h *History) Delete(ctx context.Context, id string) error {
	if h.db == nil {
		return nil
	}
	if _, err := h.db.ExecContext(ctx, `DELETE FROM conversations WHERE id = ?`, id); err != nil {
		return err
	}
	return h.Reload(ctx)
}

func (h *History) PruneBefore(ctx context.Context, cutoff int64) error {
	if h.db == nil {
		return nil
	}
	result, err := h.db.ExecContext(ctx, `DELETE FROM conversations WHERE updated_at < ?`, cutoff)
	if err != nil {
		return err
	}
	if changed, _ := result.RowsAffected(); changed > 0 {
		_, _ = h.db.ExecContext(ctx, `VACUUM`)
	}
	return h.Reload(ctx)
}
