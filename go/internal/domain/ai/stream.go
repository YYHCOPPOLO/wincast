package ai

import (
	"bytes"
	"encoding/json"
	"strings"
)

// SSEBufferCap bounds one unfinished server-sent event frame. A provider that
// exceeds it is treated as malformed rather than allowed to grow without end.
const SSEBufferCap = 1 << 20

const malformedStream = "The provider returned malformed streaming data."

// SseParser splits a byte stream into event payloads. Empty keep-alive frames
// are ignored, CRLF and LF boundaries both work, and a boundary may span
// reads.
type SseParser struct {
	buffer     []byte
	overflowed bool
}

func (p *SseParser) Overflowed() bool { return p.overflowed }

func (p *SseParser) Feed(data []byte) []string {
	if p.overflowed {
		return nil
	}
	if len(p.buffer)+len(data) > SSEBufferCap {
		p.overflowed = true
		p.buffer = nil
		return nil
	}
	p.buffer = append(p.buffer, data...)
	var payloads []string
	for {
		start, end, ok := nextBoundary(p.buffer)
		if !ok {
			break
		}
		frame := append([]byte(nil), p.buffer[:start]...)
		p.buffer = append(p.buffer[:0], p.buffer[end:]...)
		if payload, ok := payloadIn(frame); ok {
			payloads = append(payloads, payload)
		}
	}
	return payloads
}

// Finish decodes the final frame when the connection closes without a blank
// line after it.
func (p *SseParser) Finish() []string {
	if len(p.buffer) == 0 {
		return nil
	}
	frame := p.buffer
	p.buffer = nil
	if payload, ok := payloadIn(frame); ok {
		return []string{payload}
	}
	return nil
}

func nextBoundary(buffer []byte) (start, end int, ok bool) {
	lf := bytes.Index(buffer, []byte("\n\n"))
	crlf := bytes.Index(buffer, []byte("\r\n\r\n"))
	switch {
	case lf >= 0 && crlf >= 0:
		if lf < crlf {
			return lf, lf + 2, true
		}
		return crlf, crlf + 4, true
	case lf >= 0:
		return lf, lf + 2, true
	case crlf >= 0:
		return crlf, crlf + 4, true
	default:
		return 0, 0, false
	}
}

func payloadIn(frame []byte) (string, bool) {
	text := strings.ReplaceAll(string(frame), "\r\n", "\n")
	var data []string
	for _, line := range strings.Split(text, "\n") {
		if strings.HasPrefix(line, ":") {
			continue
		}
		if line == "data" {
			data = append(data, "")
		} else if value, found := strings.CutPrefix(line, "data:"); found {
			data = append(data, strings.TrimPrefix(value, " "))
		}
	}
	if len(data) == 0 {
		return "", false
	}
	return strings.Join(data, "\n"), true
}

// EventKind is one decoded stream event.
type EventKind uint8

const (
	EventText EventKind = iota
	EventThinking
	EventUsage
	EventDone
	EventError
)

// Event is what a provider reports while streaming. Text events carry a delta.
type Event struct {
	Kind EventKind
	Text string
}

type StreamShape uint8

const (
	ShapeOpenAI StreamShape = iota
	ShapeAnthropic
)

// Decoder turns event payloads into provider-neutral events. It stops at the
// first terminal frame: Done, [DONE], a finish reason or an error.
type Decoder struct {
	shape    StreamShape
	parser   SseParser
	terminal bool
}

func NewDecoder(shape StreamShape) *Decoder {
	return &Decoder{shape: shape, parser: SseParser{}}
}

func (d *Decoder) IsTerminal() bool { return d.terminal }

func (d *Decoder) Feed(data []byte) ([]Event, error) {
	payloads := d.parser.Feed(data)
	if d.parser.Overflowed() {
		d.terminal = true
		return nil, &StreamError{Message: malformedStream}
	}
	return d.decode(payloads)
}

func (d *Decoder) Finish() ([]Event, error) {
	return d.decode(d.parser.Finish())
}

func (d *Decoder) decode(payloads []string) ([]Event, error) {
	var events []Event
	for _, payload := range payloads {
		if d.terminal {
			break
		}
		if payload == "[DONE]" {
			d.terminal = true
			events = append(events, Event{Kind: EventDone})
			continue
		}
		var (
			decoded []Event
			err     error
		)
		if d.shape == ShapeAnthropic {
			decoded, err = d.decodeAnthropic(payload)
		} else {
			decoded, err = d.decodeOpenAI(payload)
		}
		if err != nil {
			return events, err
		}
		events = append(events, decoded...)
	}
	return events, nil
}

// StreamError is a provider or protocol failure suitable for showing to the
// user. It never includes a key.
type StreamError struct{ Message string }

func (e *StreamError) Error() string { return e.Message }

func (d *Decoder) decodeOpenAI(payload string) ([]Event, error) {
	var chunk struct {
		Choices []struct {
			Delta struct {
				Content          *string `json:"content"`
				Reasoning        string  `json:"reasoning"`
				ReasoningDetails []struct {
					Text string `json:"text"`
				} `json:"reasoning_details"`
			} `json:"delta"`
			FinishReason *string `json:"finish_reason"`
		} `json:"choices"`
		Usage json.RawMessage `json:"usage"`
		Error *struct {
			Message string `json:"message"`
		} `json:"error"`
	}
	if err := json.Unmarshal([]byte(payload), &chunk); err != nil {
		d.terminal = true
		return nil, &StreamError{Message: malformedStream}
	}
	if chunk.Error != nil && chunk.Error.Message != "" {
		d.terminal = true
		return nil, &StreamError{Message: chunk.Error.Message}
	}
	var events []Event
	if len(chunk.Choices) > 0 {
		delta := chunk.Choices[0].Delta
		if delta.Content != nil && *delta.Content != "" {
			events = append(events, Event{Kind: EventText, Text: *delta.Content})
		} else if hasOpenAIReasoning(delta.Reasoning, delta.ReasoningDetails) {
			events = append(events, Event{Kind: EventThinking})
		}
		if reason := chunk.Choices[0].FinishReason; reason != nil && *reason != "" {
			d.terminal = true
			events = append(events, Event{Kind: EventDone})
		}
	}
	if len(chunk.Usage) > 0 && string(chunk.Usage) != "null" {
		events = append(events, Event{Kind: EventUsage})
	}
	return events, nil
}

func hasOpenAIReasoning(text string, details []struct {
	Text string `json:"text"`
}) bool {
	if strings.TrimSpace(text) != "" {
		return true
	}
	for _, detail := range details {
		if strings.TrimSpace(detail.Text) != "" {
			return true
		}
	}
	return false
}

func (d *Decoder) decodeAnthropic(payload string) ([]Event, error) {
	var event struct {
		Type  string `json:"type"`
		Delta struct {
			Type string `json:"type"`
			Text string `json:"text"`
		} `json:"delta"`
		Error struct {
			Type string `json:"type"`
		} `json:"error"`
	}
	if err := json.Unmarshal([]byte(payload), &event); err != nil {
		d.terminal = true
		return nil, &StreamError{Message: malformedStream}
	}
	switch event.Type {
	case "content_block_delta":
		switch event.Delta.Type {
		case "text_delta":
			if event.Delta.Text == "" {
				return nil, nil
			}
			return []Event{{Kind: EventText, Text: event.Delta.Text}}, nil
		case "thinking_delta":
			return []Event{{Kind: EventThinking}}, nil
		default:
			return nil, nil
		}
	case "message_start", "message_delta":
		return []Event{{Kind: EventUsage}}, nil
	case "message_stop":
		d.terminal = true
		return []Event{{Kind: EventDone}}, nil
	case "error":
		d.terminal = true
		return nil, &StreamError{Message: anthropicErrorMessage(event.Error.Type)}
	default:
		return nil, nil
	}
}

func anthropicErrorMessage(kind string) string {
	switch kind {
	case "authentication_error":
		return "API key rejected — check it in Settings."
	case "rate_limit_error":
		return "Rate limit reached — try again later."
	default:
		return "The provider stopped the response with an error."
	}
}

// Role is one chat author.
type Role string

const (
	RoleSystem    Role = "system"
	RoleUser      Role = "user"
	RoleAssistant Role = "assistant"
)

func ParseRole(raw string) (Role, bool) {
	switch Role(raw) {
	case RoleSystem, RoleUser, RoleAssistant:
		return Role(raw), true
	}
	return "", false
}

// Message is one request transcript entry.
type Message struct {
	Role Role
	Text string
}

func UserMessage(text string) Message      { return Message{Role: RoleUser, Text: text} }
func AssistantMessage(text string) Message { return Message{Role: RoleAssistant, Text: text} }

// Request is one completion request.
type Request struct {
	Instructions    string
	Messages        []Message
	MaxOutputTokens int
	WebSearch       bool
}

func NewRequest(messages []Message) Request {
	return Request{Messages: messages, MaxOutputTokens: 4096}
}

// OpenAIBody builds an OpenAI-compatible chat completion body. OpenRouter web
// search is a plugin and is only added there, never to other providers.
func OpenAIBody(model, instructions string, messages []Message, webSearch, openRouter bool, maxTokens int) map[string]any {
	payload := make([]map[string]string, 0, len(messages)+1)
	if text := strings.TrimSpace(instructions); text != "" {
		payload = append(payload, map[string]string{"role": "system", "content": text})
	}
	for _, message := range messages {
		text := strings.TrimSpace(message.Text)
		if text == "" {
			continue
		}
		payload = append(payload, map[string]string{"role": string(message.Role), "content": text})
	}
	body := map[string]any{
		"model":      model,
		"messages":   payload,
		"stream":     true,
		"max_tokens": maxTokens,
	}
	if webSearch && openRouter {
		body["plugins"] = []map[string]string{{"id": "web"}}
	}
	return body
}

// AnthropicBody builds a Messages API body. System text is folded into the
// top-level system field because the Messages API has no system turn.
func AnthropicBody(model, instructions string, messages []Message, maxTokens int) map[string]any {
	system := make([]string, 0, 2)
	if text := strings.TrimSpace(instructions); text != "" {
		system = append(system, text)
	}
	payload := make([]map[string]string, 0, len(messages))
	for _, message := range messages {
		text := strings.TrimSpace(message.Text)
		if text == "" {
			continue
		}
		if message.Role == RoleSystem {
			system = append(system, text)
			continue
		}
		payload = append(payload, map[string]string{"role": string(message.Role), "content": text})
	}
	body := map[string]any{
		"model":      model,
		"messages":   payload,
		"max_tokens": maxTokens,
		"stream":     true,
	}
	if len(system) > 0 {
		body["system"] = strings.Join(system, "\n\n")
	}
	return body
}
