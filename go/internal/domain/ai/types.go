// Package ai holds the pure AI contract: providers, connections, endpoint
// policy, streaming frames, chat sessions and quick action prompts. It never
// reads a disk, a clock or the network.
package ai

import "strings"

// ProviderKind is the saved provider discriminator. The values match the
// existing settings file, so a migrated connection keeps its meaning.
type ProviderKind string

const (
	ProviderOpenAI           ProviderKind = "openAI"
	ProviderAnthropic        ProviderKind = "anthropic"
	ProviderGemini           ProviderKind = "gemini"
	ProviderOpenRouter       ProviderKind = "openRouter"
	ProviderOpenAICompatible ProviderKind = "openAICompatible"
)

// Title is the name shown in settings and menus.
func (p ProviderKind) Title() string {
	switch p {
	case ProviderAnthropic:
		return "Anthropic Claude"
	case ProviderGemini:
		return "Google Gemini"
	case ProviderOpenRouter:
		return "OpenRouter"
	case ProviderOpenAICompatible:
		return "OpenAI Compatible"
	default:
		return "OpenAI API"
	}
}

// DefaultBaseURL is the preset the connection editor starts from.
func (p ProviderKind) DefaultBaseURL() string {
	switch p {
	case ProviderAnthropic:
		return "https://api.anthropic.com"
	case ProviderGemini:
		return "https://generativelanguage.googleapis.com/v1beta/openai"
	case ProviderOpenRouter:
		return "https://openrouter.ai/api/v1"
	default:
		return "https://api.openai.com/v1"
	}
}

func (p ProviderKind) IsAnthropic() bool { return p == ProviderAnthropic }

// Valid reports whether a saved provider is one this build understands.
func (p ProviderKind) Valid() bool {
	switch p {
	case ProviderOpenAI, ProviderAnthropic, ProviderGemini, ProviderOpenRouter, ProviderOpenAICompatible:
		return true
	}
	return false
}

// Providers lists the providers the connection editor offers, in menu order.
var Providers = []ProviderKind{
	ProviderOpenAI,
	ProviderAnthropic,
	ProviderGemini,
	ProviderOpenRouter,
	ProviderOpenAICompatible,
}

// Connection is one saved API endpoint. ID is the DPAPI entropy and key file
// name; changing provider or host forgets the key rather than sending it to a
// different destination.
type Connection struct {
	ID           string       `json:"id"`
	Name         string       `json:"name"`
	Provider     ProviderKind `json:"provider"`
	BaseURL      string       `json:"baseURL"`
	Models       []string     `json:"models"`
	VisionModels []string     `json:"visionModels"`
}

func NewConnection(p ProviderKind) Connection {
	return Connection{
		Name:         "",
		Provider:     p,
		BaseURL:      p.DefaultBaseURL(),
		Models:       []string{},
		VisionModels: []string{},
	}
}

func (c Connection) Title() string {
	if trimmed := strings.TrimSpace(c.Name); trimmed != "" {
		return trimmed
	}
	return c.Provider.Title()
}

// Normalize keeps JSON arrays non-null so older application versions can read
// the file if a migrated settings file is reopened there.
func (c Connection) Normalize() Connection {
	if c.Models == nil {
		c.Models = []string{}
	}
	if c.VisionModels == nil {
		c.VisionModels = []string{}
	}
	return c
}

// ShouldForgetKey reports whether a retargeted connection must drop its saved
// key. A completion path is not a new destination; a different host is.
func ShouldForgetKey(previous, next Connection) bool {
	return previous.Provider != next.Provider || !SameDestination(previous.BaseURL, next.BaseURL)
}

// SelectionKind distinguishes an API connection from the unavailable bundled
// subscription entry. The saved values match the existing settings file.
type SelectionKind string

const (
	SelectionAPI     SelectionKind = "api"
	SelectionChatGPT SelectionKind = "chatGpt"
)

// ModelSelection names the default model. API selections carry the connection
// ID; the ChatGPT variant is kept readable but is not offered by this build.
type ModelSelection struct {
	Kind       SelectionKind `json:"kind"`
	Connection string        `json:"connection,omitempty"`
	Model      string        `json:"model"`
	Effort     string        `json:"effort,omitempty"`
}

func APISelection(connection, model string) ModelSelection {
	return ModelSelection{Kind: SelectionAPI, Connection: connection, Model: model}
}

func (s ModelSelection) Valid() bool {
	if strings.TrimSpace(s.Model) == "" {
		return false
	}
	if s.Kind == SelectionAPI {
		return strings.TrimSpace(s.Connection) != ""
	}
	return s.Kind == SelectionChatGPT
}

// CompletionEndpoint appends the provider's streaming path unless the saved
// base URL already names it.
func CompletionEndpoint(p ProviderKind, baseURL string) string {
	base := strings.TrimRight(strings.TrimSpace(baseURL), "/")
	if p.IsAnthropic() {
		if strings.HasSuffix(base, "/messages") {
			return base
		}
		if strings.HasSuffix(base, "/v1") {
			return base + "/messages"
		}
		return base + "/v1/messages"
	}
	if strings.HasSuffix(base, "/chat/completions") {
		return base
	}
	return base + "/chat/completions"
}
