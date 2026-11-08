package ai

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"strings"
	"time"

	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
)

const (
	streamIdleTimeout = 90 * time.Second
	errorBodyLimit    = 8192
)

var httpClient = &http.Client{
	Transport: &http.Transport{
		Proxy:                 http.ProxyFromEnvironment,
		DialContext:           (&net.Dialer{Timeout: 10 * time.Second, KeepAlive: 30 * time.Second}).DialContext,
		TLSHandshakeTimeout:   10 * time.Second,
		ResponseHeaderTimeout: 30 * time.Second,
		ForceAttemptHTTP2:     true,
		MaxIdleConns:          4,
		MaxIdleConnsPerHost:   2,
		IdleConnTimeout:       60 * time.Second,
	},
}

// Provider streams one completion. Key is only used in the request header and
// never logged or returned in an error.
type Provider struct {
	Kind     domainai.ProviderKind
	Endpoint string
	Model    string
	Key      string
}

// Stream sends the request and reports deltas through sink until Done, Error
// or cancellation. It is safe to call from a worker goroutine.
func (p *Provider) Stream(ctx context.Context, req domainai.Request, sink func(domainai.Event)) {
	if sink == nil {
		return
	}
	if ctx == nil {
		ctx = context.Background()
	}
	maxTokens := req.MaxOutputTokens
	if maxTokens <= 0 {
		maxTokens = 4096
	}
	var body any
	shape := domainai.ShapeOpenAI
	if p.Kind.IsAnthropic() {
		shape = domainai.ShapeAnthropic
		body = domainai.AnthropicBody(p.Model, req.Instructions, req.Messages, maxTokens)
	} else {
		body = domainai.OpenAIBody(p.Model, req.Instructions, req.Messages,
			req.WebSearch, p.Kind == domainai.ProviderOpenRouter, maxTokens)
	}
	encoded, err := json.Marshal(body)
	if err != nil {
		sink(domainai.Event{Kind: domainai.EventError, Text: "Could not encode the AI request."})
		return
	}
	streamCtx, cancel := context.WithCancel(ctx)
	defer cancel()
	request, err := http.NewRequestWithContext(streamCtx, http.MethodPost, p.Endpoint, bytes.NewReader(encoded))
	if err != nil {
		sink(domainai.Event{Kind: domainai.EventError, Text: "The provider endpoint is invalid."})
		return
	}
	request.Header.Set("Content-Type", "application/json")
	request.Header.Set("Accept", "text/event-stream")
	if p.Kind.IsAnthropic() {
		if p.Key != "" {
			request.Header.Set("x-api-key", p.Key)
		}
		request.Header.Set("anthropic-version", "2023-06-01")
	} else {
		if p.Key != "" {
			request.Header.Set("Authorization", "Bearer "+p.Key)
		}
		switch p.Kind {
		case domainai.ProviderGemini:
			request.Header.Set("x-goog-api-client", "tinycast-oai/0.10.2")
		case domainai.ProviderOpenRouter:
			request.Header.Set("X-OpenRouter-Title", "Tinycast")
		}
	}
	response, err := httpClient.Do(request)
	if err != nil {
		if streamCtx.Err() != nil {
			return
		}
		sink(domainai.Event{Kind: domainai.EventError, Text: connectionError(err)})
		return
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		detail, _ := io.ReadAll(io.LimitReader(response.Body, errorBodyLimit))
		sink(domainai.Event{Kind: domainai.EventError, Text: statusError(response.StatusCode, detail, p.Key)})
		return
	}
	idle := time.AfterFunc(streamIdleTimeout, cancel)
	defer idle.Stop()
	decoder := domainai.NewDecoder(shape)
	buffer := make([]byte, 32*1024)
	for {
		count, readErr := response.Body.Read(buffer)
		if count > 0 {
			idle.Reset(streamIdleTimeout)
			events, decodeErr := decoder.Feed(buffer[:count])
			for _, event := range events {
				sink(event)
			}
			if decodeErr != nil {
				sink(domainai.Event{Kind: domainai.EventError, Text: decodeErr.Error()})
				return
			}
			if decoder.IsTerminal() {
				return
			}
		}
		if readErr != nil {
			if readErr == io.EOF {
				break
			}
			if streamCtx.Err() != nil {
				return
			}
			sink(domainai.Event{Kind: domainai.EventError, Text: "The connection was interrupted."})
			return
		}
	}
	events, err := decoder.Finish()
	for _, event := range events {
		sink(event)
	}
	if err != nil {
		sink(domainai.Event{Kind: domainai.EventError, Text: err.Error()})
		return
	}
	if !decoder.IsTerminal() {
		sink(domainai.Event{
			Kind: domainai.EventError,
			Text: "The connection closed before the response completed.",
		})
	}
}

// CloseHTTPIdleConnections releases keep-alive sockets when the application
// exits; active streams are canceled through their own contexts first.
func CloseHTTPIdleConnections() {
	httpClient.CloseIdleConnections()
}

func connectionError(err error) string {
	var netErr net.Error
	if errors.As(err, &netErr) && netErr.Timeout() {
		return "The provider did not respond in time."
	}
	return "The provider could not be reached."
}

// statusError extracts a readable provider message without ever echoing the
// saved key. The body is bounded before it is inspected.
func statusError(status int, body []byte, key string) string {
	if status == http.StatusUnauthorized || status == http.StatusForbidden {
		return "API key rejected — check it in Settings."
	}
	if status == http.StatusTooManyRequests {
		return "Rate limit reached — try again later."
	}
	message := ""
	var envelope struct {
		Error struct {
			Message string `json:"message"`
		} `json:"error"`
	}
	if json.Unmarshal(body, &envelope) == nil {
		message = strings.TrimSpace(envelope.Error.Message)
	}
	if message == "" {
		message = strings.TrimSpace(string(body))
	}
	if key != "" {
		message = strings.ReplaceAll(message, key, "[redacted]")
	}
	if index := strings.IndexByte(message, '\n'); index >= 0 {
		message = message[:index]
	}
	message = strings.TrimSpace(message)
	if message == "" {
		return fmt.Sprintf("The provider returned HTTP %d.", status)
	}
	runes := []rune(message)
	if len(runes) > 300 {
		message = string(runes[:300])
	}
	return message
}
