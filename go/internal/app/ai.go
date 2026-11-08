package app

import (
	"context"
	"encoding/json"
	"log"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	domainai "github.com/YYHCOPPOLO/wincast/go/internal/domain/ai"
	aifeature "github.com/YYHCOPPOLO/wincast/go/internal/features/ai"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
)

const aiMessagePlaceholder = "Response interrupted."

func (c *controller) newAIID() string {
	id, err := storage.NewID()
	if err != nil {
		return "chat-" + time.Now().Format("20060102150405.000000000")
	}
	return id
}

// loadAISettings keeps the existing settings keys and preserves structured
// values it cannot parse instead of replacing them with defaults.
func (c *controller) loadAISettings() {
	document := c.settingsDocument
	c.aiConnEditing = -1
	if document == nil {
		return
	}
	c.aiEnabled = document.Bool("aiEnabled", false)
	c.aiWebSearch = document.Bool("aiWebSearch", false)
	c.aiSystemPrompt = document.String("aiSystemPrompt", "")
	c.aiSystemPromptDraft = c.aiSystemPrompt
	c.aiSystemPromptEnabled = document.Bool("aiSystemPromptEnabled", true)
	c.aiRetention = int64(document.Int("aiRetentionDays", 30))
	c.aiOpensTo = int64(document.Int("aiOpensTo", 0))
	c.aiNewChatAfter = int64(document.Int("aiNewChatAfterMinutes", 10))
	c.quickActionsEnabled = document.Bool("quickActionsEnabled", false)
	c.quickActionLanguage = document.String("quickActionLanguage", "")
	if raw, ok := document.Raw("aiConnections"); ok {
		var connections []domainai.Connection
		if json.Unmarshal(raw, &connections) == nil {
			for i := range connections {
				connections[i] = connections[i].Normalize()
			}
			c.aiConnections = connections
		} else {
			c.aiConfigError = c.text("AI 连接配置无法读取，已忽略", "AI connections are unreadable; they were ignored")
		}
	}
	c.aiDefaultModel = decodeAISelection(document, "aiDefaultModel", &c.aiConfigError, c)
	c.quickActionModel = decodeAISelection(document, "quickActionModel", &c.aiConfigError, c)
	c.clampAI()
}

func decodeAISelection(document *storage.Document, key string, configError *string, c *controller) *domainai.ModelSelection {
	raw, ok := document.Raw(key)
	if !ok || string(raw) == "null" {
		return nil
	}
	var selection domainai.ModelSelection
	if err := json.Unmarshal(raw, &selection); err != nil {
		if *configError == "" {
			*configError = c.text("AI 模型配置无法读取，已忽略", "AI model settings are unreadable; they were ignored")
		}
		return nil
	}
	return &selection
}

func (c *controller) clampAI() {
	switch c.aiRetention {
	case 7, 30, 90, -1:
	default:
		c.aiRetention = 30
	}
	if c.aiOpensTo != 0 {
		c.aiOpensTo = 0
	}
	switch c.quickActionLanguage {
	case "", "Spanish", "French", "German", "English":
	default:
		c.quickActionLanguage = ""
	}
	if c.aiDefaultModel != nil && !c.aiDefaultModel.Valid() {
		c.aiDefaultModel = nil
	}
	if c.quickActionModel != nil && !c.quickActionModel.Valid() {
		c.quickActionModel = nil
	}
}

func (c *controller) startAI() {
	if !c.aiEnabled {
		return
	}
	c.openAIHistory()
}

func (c *controller) openAIHistory() {
	if c.aiHistory == nil || c.aiHistory.Available() {
		return
	}
	if err := c.aiHistory.Open(c.ctx); err != nil {
		log.Printf("AI history open failed: %v", err)
		c.aiConfigError = c.text("无法保存 AI 对话历史", "Chat history is unavailable")
		return
	}
	if c.aiRetention > 0 {
		cutoff := time.Now().Unix() - c.aiRetention*86_400
		if err := c.aiHistory.PruneBefore(c.ctx, cutoff); err != nil {
			log.Printf("AI history prune failed: %v", err)
		}
	}
}

func (c *controller) closeAIHistory() {
	if c.aiHistory != nil {
		c.aiHistory.Close()
	}
}

func (c *controller) saveAISession() {
	if c.aiHistory == nil || !c.aiHistory.Available() {
		return
	}
	if err := c.aiHistory.Save(c.ctx, c.aiSession); err != nil {
		log.Printf("AI history save failed: %v", err)
		c.aiNotice = c.text("对话未能保存", "The conversation was not saved")
	}
}

// openAIChat is the launcher entry. It resumes according to the saved policy
// and paints the transcript before any refresh.
func (c *controller) openAIChat() {
	if c.closing {
		return
	}
	if !c.aiEnabled {
		c.view.PendingToast = c.text("AI 已关闭", "AI is disabled")
		return
	}
	if !c.panel.IsVisible() {
		c.target = windows.ForegroundTarget()
	}
	c.applyAIOpenPolicy()
	c.view.SetModeAI(c.aiDraft)
	c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
	c.view.SetAIStreaming(c.aiStreaming)
	c.showPanel(false)
	c.panel.Update(func() {})
}

func (c *controller) applyAIOpenPolicy() {
	if c.aiStreaming {
		return
	}
	resident := len(c.aiSession.Messages) > 0
	last := c.aiSession.UpdatedAt
	if !resident && c.aiHistory != nil && c.aiHistory.Available() {
		if conversations := c.aiHistory.Conversations(); len(conversations) > 0 {
			last = conversations[0].UpdatedAt
		}
	}
	if !domainai.ShouldResume(c.aiOpensTo, c.aiNewChatAfter, last, time.Now().Unix()) {
		if resident {
			c.startNewAIChat()
		}
		return
	}
	if resident {
		return
	}
	if c.aiHistory == nil || !c.aiHistory.Available() {
		return
	}
	conversations := c.aiHistory.Conversations()
	if len(conversations) == 0 {
		return
	}
	if session, ok := c.aiHistory.Session(c.ctx, conversations[0].ID); ok {
		c.aiSession = session
		c.aiGeneration++
		c.aiNotice = ""
		c.aiThinking = false
	} else {
		log.Printf("AI history latest conversation could not be loaded")
	}
}

func (c *controller) startNewAIChat() {
	if c.aiStreaming {
		c.cancelAIStream()
	}
	c.aiGeneration++
	c.aiSession = domainai.NewSession(c.newAIID(), time.Now().Unix())
	c.aiNotice = ""
	c.aiThinking = false
	c.aiDraft = ""
	c.view.ClearAIComposer()
	c.view.SetAIChat(c.aiSession.Messages, "", false, false)
	c.view.SetAIStreaming(false)
}

func (c *controller) showAIHistory() {
	if !c.aiEnabled {
		return
	}
	c.aiDraft = c.view.Query
	c.view.SetModeAIHistory()
	c.view.SetAIHistory(c.aiHistorySearch(""))
	c.view.SetAIStreaming(c.aiStreaming)
	c.panel.Update(func() {})
}

func (c *controller) aiHistorySearch(query string) []domainai.Conversation {
	if c.aiHistory == nil || !c.aiHistory.Available() {
		return nil
	}
	return c.aiHistory.Search(query)
}

func (c *controller) openAIConversation(id string) {
	if c.aiHistory == nil || !c.aiHistory.Available() {
		return
	}
	session, ok := c.aiHistory.Session(c.ctx, id)
	if !ok {
		c.view.PendingToast = c.text("无法打开这段对话", "Could not open the conversation")
		return
	}
	c.cancelAIStream()
	c.aiSession = session
	c.aiGeneration++
	c.aiNotice = ""
	c.aiThinking = false
	c.aiStreaming = false
	c.view.SetModeAI(c.aiDraft)
	c.view.SetAIChat(c.aiSession.Messages, "", false, false)
	c.view.SetAIStreaming(false)
	c.showPanel(false)
	c.panel.Update(func() {})
}

// sendAIMessage validates the model and key before mutating the transcript, so
// a failed send never leaves a duplicate user turn.
func (c *controller) sendAIMessage(text string) {
	message := strings.TrimSpace(text)
	if message == "" {
		return
	}
	if c.aiStreaming {
		c.cancelAIStream()
		return
	}
	if !c.aiEnabled {
		c.aiNotice = c.text("AI 已关闭", "AI is disabled")
		c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
		return
	}
	selection := c.aiDefaultModel
	if selection == nil {
		c.aiNotice = c.text("请在设置中选择默认 AI 模型。", "Choose a default AI model in Settings.")
		c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
		return
	}
	if selection.Kind == domainai.SelectionChatGPT {
		c.aiNotice = c.text("ChatGPT 订阅暂未提供；请选择 API 连接。",
			"ChatGPT chat is not available yet. Choose an API connection.")
		c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
		return
	}
	connection, ok := c.aiConnection(selection.Connection)
	if !ok {
		c.aiNotice = c.text("请在设置中选择 API 连接。", "Choose an API connection in Settings.")
		c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
		return
	}
	provider, err := aifeature.ProviderFor(connection, selection.Model, c.aiKeys)
	if err != nil {
		c.aiNotice = c.aiErrorText(err)
		c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
		return
	}
	now := time.Now().Unix()
	c.aiSession.Append(domainai.NewUserMessage(c.newAIID(), message, now))
	c.aiSession.Append(domainai.NewAssistantMessage(c.newAIID(), now))
	c.aiNotice = ""
	c.aiThinking = false
	c.aiStreaming = true
	c.saveAISession()
	request := domainai.NewRequest(c.aiSession.RequestMessages(domainai.DefaultTextBudget))
	request.Instructions = domainai.ComposeInstructions(c.aiSystemPrompt, c.aiSystemPromptEnabled)
	request.WebSearch = c.aiWebSearch
	c.aiDraft = ""
	c.view.ClearAIComposer()
	c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, true)
	c.view.SetAIStreaming(true)
	c.streamAI(provider, request)
}

func (c *controller) streamAI(provider *aifeature.Provider, request domainai.Request) {
	generation := c.aiGeneration + 1
	c.aiGeneration = generation
	streamCtx, cancel := context.WithCancel(c.ctx)
	c.aiCancel = cancel
	events := make(chan domainai.Event, 256)
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		provider.Stream(streamCtx, request, func(event domainai.Event) {
			select {
			case events <- event:
			case <-streamCtx.Done():
			}
		})
		close(events)
	}()
	c.workers.Add(1)
	go func() {
		defer c.workers.Done()
		ticker := time.NewTicker(50 * time.Millisecond)
		defer ticker.Stop()
		var pending []domainai.Event
		flush := func() bool {
			if len(pending) == 0 {
				return false
			}
			batch := pending
			pending = nil
			terminal := false
			for _, event := range batch {
				if event.Kind == domainai.EventDone || event.Kind == domainai.EventError {
					terminal = true
				}
			}
			c.panel.Update(func() {
				if c.closing || generation != c.aiGeneration {
					return
				}
				c.applyAIEvents(batch)
				c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
				c.view.SetAIStreaming(c.aiStreaming)
			})
			return terminal
		}
		for {
			select {
			case <-streamCtx.Done():
				return
			case <-c.ctx.Done():
				return
			case event, ok := <-events:
				if !ok {
					flush()
					return
				}
				pending = append(pending, event)
				if event.Kind == domainai.EventDone || event.Kind == domainai.EventError {
					flush()
					return
				}
			case <-ticker.C:
				if flush() {
					return
				}
			}
		}
	}()
}

// applyAIEvents mutates UI-owned chat state from the panel update closure.
func (c *controller) applyAIEvents(events []domainai.Event) bool {
	for _, event := range events {
		switch event.Kind {
		case domainai.EventText:
			c.aiThinking = false
			if len(c.aiSession.Messages) > 0 {
				last := &c.aiSession.Messages[len(c.aiSession.Messages)-1]
				if last.Role == domainai.ChatAssistant && last.State == domainai.StateStreaming {
					last.Text += event.Text
				}
			}
		case domainai.EventThinking:
			c.aiThinking = true
		case domainai.EventUsage:
		case domainai.EventDone:
			c.finishAI(domainai.StateComplete, "")
			return true
		case domainai.EventError:
			c.finishAI(domainai.StateFailed, event.Text)
			return true
		}
	}
	return false
}

func (c *controller) finishAI(state domainai.ChatState, fallback string) {
	c.aiStreaming = false
	c.aiThinking = false
	if c.aiCancel != nil {
		c.aiCancel()
		c.aiCancel = nil
	}
	if len(c.aiSession.Messages) > 0 {
		last := &c.aiSession.Messages[len(c.aiSession.Messages)-1]
		if last.Role == domainai.ChatAssistant && last.State == domainai.StateStreaming {
			if last.Text == "" && fallback != "" {
				last.Text = fallback
			}
			last.State = state
		}
	}
	if state == domainai.StateFailed && fallback != "" {
		c.aiNotice = c.aiErrorTextText(fallback)
	}
	c.saveAISession()
	c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, false)
	c.view.SetAIStreaming(false)
}

func (c *controller) cancelAIStream() {
	if !c.aiStreaming {
		return
	}
	c.aiGeneration++
	c.finishAI(domainai.StateFailed, "Cancelled")
	c.panel.Update(func() {})
}

func (c *controller) aiConnection(id string) (domainai.Connection, bool) {
	for _, connection := range c.aiConnections {
		if connection.ID == id {
			return connection, true
		}
	}
	return domainai.Connection{}, false
}

func (c *controller) aiModelChoices() []domainai.ModelSelection {
	choices := make([]domainai.ModelSelection, 0, 8)
	for _, connection := range c.aiConnections {
		for _, model := range connection.Models {
			if strings.TrimSpace(model) == "" {
				continue
			}
			choices = append(choices, domainai.APISelection(connection.ID, model))
		}
	}
	return choices
}

func (c *controller) aiModelLabel() string {
	if c.aiDefaultModel == nil {
		return c.text("选择模型", "Choose model")
	}
	label := c.aiDefaultModel.Model
	if connection, ok := c.aiConnection(c.aiDefaultModel.Connection); ok {
		label = connection.Title() + " · " + label
	}
	if c.aiDefaultModel.Kind == domainai.SelectionChatGPT {
		label = "ChatGPT · " + label
	}
	return label
}

func (c *controller) setAIDefaultModel(selection *domainai.ModelSelection) {
	if selection != nil && !selection.Valid() {
		return
	}
	c.aiDefaultModel = selection
	c.persistAISettings()
	c.refreshAIChatView()
	c.refreshSettingsWindow()
}

// refreshAIChatView keeps the palette header, transcript and streaming flag in
// step after a settings or connection change.
func (c *controller) refreshAIChatView() {
	if c.view == nil {
		return
	}
	c.view.AIAvailable = c.aiEnabled
	c.view.AIModelLabel = c.aiModelLabel()
	c.view.SetAIChat(c.aiSession.Messages, c.aiNotice, c.aiThinking, c.aiStreaming)
	c.view.SetAIStreaming(c.aiStreaming)
}

func (c *controller) cycleAIDefaultModel() {
	choices := c.aiModelChoices()
	if len(choices) == 0 {
		c.settingsStatus = c.text("请先添加包含模型的 AI 连接", "Add an AI connection with a model first")
		return
	}
	c.setAIDefaultModel(nextAISelection(c.aiDefaultModel, choices))
}

func nextAISelection(current *domainai.ModelSelection, choices []domainai.ModelSelection) *domainai.ModelSelection {
	if len(choices) == 0 {
		return nil
	}
	if current == nil {
		return &choices[0]
	}
	for index, choice := range choices {
		if choice.Kind == current.Kind && choice.Connection == current.Connection && choice.Model == current.Model {
			next := choices[(index+1)%len(choices)]
			return &next
		}
	}
	return &choices[0]
}

func (c *controller) setAIEnabled(enabled bool) {
	if c.aiEnabled == enabled {
		return
	}
	c.aiEnabled = enabled
	if !enabled {
		c.cancelAIStream()
		c.closeAIHistory()
		c.aiSession = domainai.NewSession(c.newAIID(), time.Now().Unix())
		c.aiNotice = ""
		c.aiThinking = false
		c.aiDraft = ""
		c.view.SetAIChat(nil, "", false, false)
		c.view.SetAIStreaming(false)
		if c.view.Mode == palette.ModeAI || c.view.Mode == palette.ModeAIHistory {
			c.view.SetModeLauncher()
		}
	} else {
		c.openAIHistory()
	}
	c.persistAISettings()
	c.refreshAIChatView()
	c.refreshExtras()
	c.refreshSettingsWindow()
}

func (c *controller) setAIWebSearch(enabled bool) {
	c.aiWebSearch = enabled
	c.persistAISettings()
}

func (c *controller) setAISystemPromptEnabled(enabled bool) {
	c.aiSystemPromptEnabled = enabled
	c.persistAISettings()
}

func (c *controller) applyAISystemPrompt(text string) {
	c.aiSystemPrompt = text
	c.aiSystemPromptDraft = text
	c.persistAISettings()
	c.settingsStatus = c.text("系统提示已保存", "System prompt saved")
}

func (c *controller) cycleAIRetention() {
	switch c.aiRetention {
	case 7:
		c.aiRetention = 30
	case 30:
		c.aiRetention = 90
	case 90:
		c.aiRetention = -1
	default:
		c.aiRetention = 7
	}
	c.persistAISettings()
}

func (c *controller) cycleAIOpensTo() {
	if c.aiOpensTo == 0 {
		c.aiOpensTo = 1
	} else {
		c.aiOpensTo = 0
	}
	c.persistAISettings()
}

func (c *controller) setQuickActionsEnabled(enabled bool) {
	if c.quickActionsEnabled == enabled {
		return
	}
	c.quickActionsEnabled = enabled
	if !enabled {
		c.cancelQuickAction()
	}
	c.persistAISettings()
	c.refreshExtras()
	c.refreshSettingsWindow()
}

func (c *controller) cycleQuickActionLanguage() {
	switch c.quickActionLanguage {
	case "", "English":
		c.quickActionLanguage = "Spanish"
	case "Spanish":
		c.quickActionLanguage = "French"
	case "French":
		c.quickActionLanguage = "German"
	case "German":
		c.quickActionLanguage = "English"
	default:
		c.quickActionLanguage = ""
	}
	c.persistAISettings()
}

func (c *controller) cycleQuickActionModel() {
	choices := c.aiModelChoices()
	index := -1
	for i, choice := range choices {
		if c.quickActionModel != nil && choice.Kind == c.quickActionModel.Kind &&
			choice.Connection == c.quickActionModel.Connection && choice.Model == c.quickActionModel.Model {
			index = i
			break
		}
	}
	if c.quickActionModel == nil {
		if len(choices) == 0 {
			c.settingsStatus = c.text("请先添加 AI 连接", "Add an AI connection first")
			return
		}
		next := choices[0]
		c.quickActionModel = &next
	} else if index >= 0 && index+1 < len(choices) {
		next := choices[index+1]
		c.quickActionModel = &next
	} else {
		c.quickActionModel = nil
	}
	c.persistAISettings()
}

// persistAISettings writes scalar keys always. Structured values are skipped
// while the loaded file still has an unreadable shape, so toggling a feature
// cannot silently replace data this build could not model.
func (c *controller) persistAISettings() {
	document, writable := c.settingDocument()
	if document == nil || !writable {
		return
	}
	_ = document.Set("aiEnabled", c.aiEnabled)
	_ = document.Set("aiWebSearch", c.aiWebSearch)
	_ = document.Set("aiSystemPrompt", c.aiSystemPrompt)
	_ = document.Set("aiSystemPromptEnabled", c.aiSystemPromptEnabled)
	_ = document.Set("aiRetentionDays", c.aiRetention)
	_ = document.Set("aiOpensTo", c.aiOpensTo)
	_ = document.Set("aiNewChatAfterMinutes", c.aiNewChatAfter)
	_ = document.Set("quickActionsEnabled", c.quickActionsEnabled)
	_ = document.Set("quickActionLanguage", c.quickActionLanguage)
	if c.aiConfigError == "" {
		connections := make([]domainai.Connection, len(c.aiConnections))
		for i, connection := range c.aiConnections {
			connections[i] = connection.Normalize()
		}
		_ = document.Set("aiConnections", connections)
		_ = document.Set("aiDefaultModel", c.aiDefaultModel)
		_ = document.Set("quickActionModel", c.quickActionModel)
	}
	c.queueSettingsSave()
}

func (c *controller) startNewAIConnection() {
	c.aiConnEditing = len(c.aiConnections)
	c.aiConnDraftProvider = domainai.ProviderOpenAI
	c.aiConnDraftURL = domainai.ProviderOpenAI.DefaultBaseURL()
	c.aiConnDraftModel = ""
	c.aiConnDraftKey = ""
	c.aiConnDraftErr = ""
}

func (c *controller) editAIConnection(index int) {
	if index < 0 || index >= len(c.aiConnections) {
		return
	}
	connection := c.aiConnections[index]
	c.aiConnEditing = index
	c.aiConnDraftProvider = connection.Provider
	c.aiConnDraftURL = connection.BaseURL
	c.aiConnDraftModel = ""
	if len(connection.Models) > 0 {
		c.aiConnDraftModel = connection.Models[0]
	}
	c.aiConnDraftKey = ""
	c.aiConnDraftErr = ""
}

func (c *controller) cancelAIConnectionEdit() {
	c.aiConnEditing = -1
	c.aiConnDraftErr = ""
}

func (c *controller) cycleAIConnectionProvider() {
	c.aiConnDraftProvider = domainai.CycleProvider(c.aiConnDraftProvider)
	if strings.TrimSpace(c.aiConnDraftURL) == "" ||
		strings.HasSuffix(strings.TrimRight(c.aiConnDraftURL, "/"), "/chat/completions") ||
		strings.HasSuffix(strings.TrimRight(c.aiConnDraftURL, "/"), "/messages") {
		c.aiConnDraftURL = c.aiConnDraftProvider.DefaultBaseURL()
	}
}

func (c *controller) saveAIConnection() {
	parsed, err := domainai.ValidateURL(c.aiConnDraftURL)
	if err != nil {
		c.aiConnDraftErr = c.endpointErrorText(err)
		return
	}
	model := strings.TrimSpace(c.aiConnDraftModel)
	index := c.aiConnEditing
	creating := index >= len(c.aiConnections)
	var previous domainai.Connection
	if !creating {
		if index < 0 {
			return
		}
		previous = c.aiConnections[index]
	}
	next := previous
	if creating {
		id, idErr := storage.NewID()
		if idErr != nil {
			c.aiConnDraftErr = c.text("无法创建连接", "Could not create the connection")
			return
		}
		next = domainai.Connection{ID: id}
	}
	next.Provider = c.aiConnDraftProvider
	next.BaseURL = parsed.Raw
	if model == "" {
		next.Models = []string{}
	} else {
		next.Models = []string{model}
	}
	next = next.Normalize()
	key := strings.TrimSpace(c.aiConnDraftKey)
	if key != "" {
		if err := c.aiKeys.Set(next.ID, key); err != nil {
			c.aiConnDraftErr = c.text("无法保存此连接。", "Couldn’t save this connection.")
			return
		}
	} else if !creating && domainai.ShouldForgetKey(previous, next) {
		c.aiKeys.Remove(previous.ID)
	}
	if creating {
		c.aiConnections = append(c.aiConnections, next)
	} else {
		c.aiConnections[index] = next
	}
	c.aiConfigError = ""
	c.persistAISettings()
	c.aiConnEditing = -1
	c.aiConnDraftKey = ""
	c.aiConnDraftErr = ""
	c.refreshAIChatView()
	c.refreshExtras()
	c.refreshSettingsWindow()
}

func (c *controller) removeAIConnection(index int) {
	if index < 0 || index >= len(c.aiConnections) {
		return
	}
	connection := c.aiConnections[index]
	c.aiKeys.Remove(connection.ID)
	c.aiConnections = append(c.aiConnections[:index], c.aiConnections[index+1:]...)
	if c.aiDefaultModel != nil && c.aiDefaultModel.Connection == connection.ID {
		c.aiDefaultModel = nil
	}
	if c.quickActionModel != nil && c.quickActionModel.Connection == connection.ID {
		c.quickActionModel = nil
	}
	if c.aiConnEditing == index {
		c.aiConnEditing = -1
	} else if c.aiConnEditing > index {
		c.aiConnEditing--
	}
	c.persistAISettings()
	c.refreshAIChatView()
	c.refreshExtras()
	c.refreshSettingsWindow()
}

func (c *controller) aiConnectionKeySaved(index int) bool {
	if index < 0 || index >= len(c.aiConnections) {
		return false
	}
	return c.aiKeys.Has(c.aiConnections[index].ID)
}

func (c *controller) aiConnectionModel(connection domainai.Connection) string {
	if len(connection.Models) == 0 {
		return c.text("未设置模型", "No model")
	}
	return strings.Join(connection.Models, ", ")
}

func (c *controller) aiRetentionLabel() string {
	switch c.aiRetention {
	case 7:
		return c.text("7 天", "7 days")
	case 90:
		return c.text("3 个月", "3 months")
	case -1:
		return c.text("永久保留", "Forever")
	default:
		return c.text("30 天", "30 days")
	}
}

func (c *controller) aiOpensToLabel() string {
	if c.aiOpensTo == 0 {
		return c.text("继续最近的对话", "Recent conversation")
	}
	return c.text("总是新建对话", "A new conversation")
}

func (c *controller) quickActionLanguageLabel() string {
	if c.quickActionLanguage == "" {
		return c.text("默认（英语）", "Default (English)")
	}
	return c.quickActionLanguage
}

func (c *controller) quickActionModelLabel() string {
	if c.quickActionModel == nil {
		return c.text("使用默认模型", "Use default model")
	}
	label := c.quickActionModel.Model
	if connection, ok := c.aiConnection(c.quickActionModel.Connection); ok {
		label = connection.Title() + " · " + label
	}
	return label
}

func (c *controller) endpointErrorText(err error) string {
	switch err {
	case domainai.ErrInsecureURL:
		return c.text("远程 AI 服务需要 HTTPS 地址。", "Remote AI providers require an HTTPS base URL.")
	case domainai.ErrInvalidURL:
		return c.text("请输入有效的服务地址。", "Enter a valid provider base URL.")
	}
	return c.text("请输入有效的服务地址。", "Enter a valid provider base URL.")
}

func (c *controller) aiErrorText(err error) string {
	if err == nil {
		return ""
	}
	switch err.Error() {
	case domainai.ErrNoKey.Error():
		return c.text("请在设置中添加 API 密钥。", "Add an API key in Settings.")
	}
	return c.aiErrorTextText(err.Error())
}

func (c *controller) aiErrorTextText(message string) string {
	switch message {
	case "Add an API key in Settings.":
		return c.text("请在设置中添加 API 密钥。", "Add an API key in Settings.")
	case "API key rejected — check it in Settings.":
		return c.text("API 密钥被拒绝，请在设置中检查。", "API key rejected — check it in Settings.")
	case "Rate limit reached — try again later.":
		return c.text("已达到速率限制，请稍后再试。", "Rate limit reached — try again later.")
	case "The provider returned malformed streaming data.":
		return c.text("服务返回了无法解析的流数据。", "The provider returned malformed streaming data.")
	case "The connection closed before the response completed.":
		return c.text("连接在回复完成前关闭。", "The connection closed before the response completed.")
	case "The connection was interrupted.":
		return c.text("连接中断。", "The connection was interrupted.")
	case "The provider did not respond in time.":
		return c.text("服务响应超时。", "The provider did not respond in time.")
	case "The provider could not be reached.":
		return c.text("无法连接到服务。", "The provider could not be reached.")
	case "Could not encode the AI request.":
		return c.text("无法编码 AI 请求。", "Could not encode the AI request.")
	case "The provider endpoint is invalid.":
		return c.text("AI 服务地址无效。", "The provider endpoint is invalid.")
	case "Unsupported AI provider.":
		return c.text("不支持该 AI 服务商。", "Unsupported AI provider.")
	case "Cancelled":
		return c.text("已取消", "Cancelled")
	case aiMessagePlaceholder:
		return c.text("回复已中断。", aiMessagePlaceholder)
	}
	return message
}

func (c *controller) copyLastAIResponse() {
	if text := c.aiSession.LastAssistantText(); text != "" {
		c.copyText(text, false)
	}
}

// aiMenu is the AI Chat action menu. Stop stays visible only while streaming;
// Copy only when a reply exists.
func (c *controller) aiMenu() {
	actions := []components.MenuAction{
		{Label: c.text("新对话", "New Chat"), Accelerator: "Ctrl+N", Click: c.newAIChatFromMenu},
		{Label: c.text("对话历史…", "Chat History…"), Click: c.showAIHistory},
		{Label: c.text("AI 设置…", "AI Settings…"), Click: c.openSettings},
	}
	if c.aiStreaming {
		actions = append(actions, components.MenuAction{
			Label: c.text("停止回复", "Stop Response"), Accelerator: "↵", Click: c.cancelAIStream,
		})
	}
	if c.aiSession.LastAssistantText() != "" {
		actions = append(actions, components.MenuAction{
			Label: c.text("复制最后一条回复", "Copy Last Response"), Click: c.copyLastAIResponse,
		})
	}
	components.Menu(actions).Popup(c.panel)
}

func (c *controller) newAIChatFromMenu() {
	c.startNewAIChat()
	c.view.SetModeAI("")
	c.view.SetAIChat(c.aiSession.Messages, "", false, false)
	if !c.panel.IsVisible() {
		c.showPanel(false)
	}
}

func (c *controller) quickActionMenu() []components.MenuAction {
	actions := make([]components.MenuAction, 0, len(domainai.AllQuickActions()))
	for _, action := range domainai.AllQuickActions() {
		quick := action
		actions = append(actions, components.MenuAction{
			Label: quick.Title(), Disabled: !c.quickActionsEnabled,
			Click: func() { c.runQuickAction(quick) },
		})
	}
	return actions
}

// aiModelMenu lets the header button pick a saved connection model without
// opening Settings.
func (c *controller) aiModelMenu() {
	choices := c.aiModelChoices()
	actions := make([]components.MenuAction, 0, len(choices))
	for _, choice := range choices {
		selection := choice
		label := selection.Model
		if connection, ok := c.aiConnection(selection.Connection); ok {
			label = connection.Title() + " · " + selection.Model
		}
		actions = append(actions, components.MenuAction{
			Label: label, Click: func() { c.setAIDefaultModel(&selection) },
			Accelerator: checkedAccelerator(c.aiDefaultModel, &selection),
		})
	}
	if len(actions) == 0 {
		actions = append(actions, components.MenuAction{
			Label: c.text("在设置中添加连接…", "Add a connection in Settings…"), Click: c.openSettings,
		})
	}
	components.Menu(actions).Popup(c.panel)
}

func checkedAccelerator(current, choice *domainai.ModelSelection) string {
	if current == nil || choice == nil {
		return ""
	}
	if current.Kind == choice.Kind && current.Connection == choice.Connection && current.Model == choice.Model {
		return "✓"
	}
	return ""
}

func (c *controller) aiHistoryChoose(index int) {
	items := c.view.AIHistoryItems
	if index < 0 || index >= len(items) {
		return
	}
	c.openAIConversation(items[index].ID)
}

// routeAIEntry opens an AI Chat or Quick Action launcher row.
func (c *controller) routeAIEntry(application domain.Application) bool {
	switch application.Kind {
	case domain.KindAI:
		c.openAIChat()
		return true
	case domain.KindQuickAction:
		if action, ok := domainai.QuickActionFromEntry(application.ID); ok {
			c.runQuickAction(action)
		}
		return true
	}
	return false
}
