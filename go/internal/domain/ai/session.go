package ai

import "strings"

// DefaultTextBudget is the request transcript budget, in bytes. The newest
// user turn is always sent whole.
const DefaultTextBudget = 100_000

type ChatRole string

const (
	ChatUser      ChatRole = "user"
	ChatAssistant ChatRole = "assistant"
)

func ParseChatRole(raw string) (ChatRole, bool) {
	switch ChatRole(raw) {
	case ChatUser, ChatAssistant:
		return ChatRole(raw), true
	}
	return "", false
}

type ChatState string

const (
	StateStreaming ChatState = "streaming"
	StateComplete  ChatState = "complete"
	StateFailed    ChatState = "failed"
)

func ParseChatState(raw string) (ChatState, bool) {
	switch ChatState(raw) {
	case StateStreaming, StateComplete, StateFailed:
		return ChatState(raw), true
	}
	return "", false
}

// ChatMessage is one persisted turn. ID is the business identity; position is
// stored separately and never used as identity.
type ChatMessage struct {
	ID     string
	Role   ChatRole
	Text   string
	State  ChatState
	SentAt int64
}

func NewChatMessage(id string, role ChatRole, text string, state ChatState, sentAt int64) ChatMessage {
	return ChatMessage{ID: id, Role: role, Text: text, State: state, SentAt: sentAt}
}

func NewUserMessage(id, text string, sentAt int64) ChatMessage {
	return NewChatMessage(id, ChatUser, text, StateComplete, sentAt)
}

func NewAssistantMessage(id string, sentAt int64) ChatMessage {
	return NewChatMessage(id, ChatAssistant, "", StateStreaming, sentAt)
}

// Conversation is the list row shown in Chat History.
type Conversation struct {
	ID           string
	Title        string
	Preview      string
	CreatedAt    int64
	UpdatedAt    int64
	MessageCount int
}

// Session is the live transcript.
type Session struct {
	ID        string
	CreatedAt int64
	UpdatedAt int64
	Messages  []ChatMessage
}

func NewSession(id string, now int64) Session {
	return Session{ID: id, CreatedAt: now, UpdatedAt: now, Messages: []ChatMessage{}}
}

func (s Session) Title() string {
	for _, message := range s.Messages {
		if message.Role == ChatUser {
			if title := summary(message.Text, 72); title != "" {
				return title
			}
			break
		}
	}
	return "New Chat"
}

func (s Session) Preview() string {
	for i := len(s.Messages) - 1; i >= 0; i-- {
		if text := summary(s.Messages[i].Text, 120); text != "" {
			return text
		}
	}
	return ""
}

func (s Session) Summary() Conversation {
	return Conversation{
		ID: s.ID, Title: s.Title(), Preview: s.Preview(),
		CreatedAt: s.CreatedAt, UpdatedAt: s.UpdatedAt, MessageCount: len(s.Messages),
	}
}

func (s *Session) Append(message ChatMessage) {
	if message.SentAt > s.UpdatedAt {
		s.UpdatedAt = message.SentAt
	}
	s.Messages = append(s.Messages, message)
}

func (s *Session) ReplaceLast(message ChatMessage, now int64) {
	if len(s.Messages) == 0 {
		return
	}
	s.Messages[len(s.Messages)-1] = message
	if now > s.UpdatedAt {
		s.UpdatedAt = now
	}
}

func (s *Session) LastAssistantText() string {
	for i := len(s.Messages) - 1; i >= 0; i-- {
		if s.Messages[i].Role == ChatAssistant && s.Messages[i].Text != "" {
			return s.Messages[i].Text
		}
	}
	return ""
}

// RequestMessages maps complete turns to the request and bounds old context.
func (s Session) RequestMessages(textBudget int) []Message {
	ready := make([]Message, 0, len(s.Messages))
	for _, message := range s.Messages {
		if message.Role != ChatUser && message.State != StateComplete {
			continue
		}
		role := RoleUser
		if message.Role == ChatAssistant {
			role = RoleAssistant
		}
		ready = append(ready, Message{Role: role, Text: message.Text})
	}
	return BoundedContext(ready, textBudget)
}

// BoundedContext keeps the newest user turn whole, then walks older turns
// newest-first until the byte budget is spent. It never starts with an
// assistant turn.
func BoundedContext(messages []Message, textBudget int) []Message {
	if textBudget < 0 {
		textBudget = 0
	}
	newest := -1
	for i := len(messages) - 1; i >= 0; i-- {
		if messages[i].Role == RoleUser {
			newest = i
			break
		}
	}
	if newest < 0 {
		return append([]Message(nil), messages...)
	}
	remaining := textBudget
	var tail []Message
	for _, message := range messages[newest+1:] {
		remaining -= len(message.Text)
		tail = append(tail, Message{Role: message.Role, Text: message.Text})
	}
	var head []Message
	for i := newest - 1; i >= 0; i-- {
		message := messages[i]
		remaining -= len(message.Text)
		if remaining < 0 {
			break
		}
		head = append(head, Message{Role: message.Role, Text: message.Text})
	}
	for len(head) > 0 && head[len(head)-1].Role == RoleAssistant {
		head = head[:len(head)-1]
	}
	for left, right := 0, len(head)-1; left < right; left, right = left+1, right-1 {
		head[left], head[right] = head[right], head[left]
	}
	head = append(head, Message{Role: messages[newest].Role, Text: messages[newest].Text})
	head = append(head, tail...)
	return head
}

func summary(text string, limit int) string {
	collapsed := strings.Join(strings.Fields(text), " ")
	if limit <= 0 {
		return ""
	}
	runes := []rune(collapsed)
	if len(runes) > limit {
		return string(runes[:limit])
	}
	return collapsed
}
