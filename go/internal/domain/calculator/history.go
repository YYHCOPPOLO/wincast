package calculator

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"strings"
)

const historyCap = 200

type HistoryEntry struct {
	Expression string `json:"expression"`
	Result     string `json:"result"`
}

type History struct {
	Entries []HistoryEntry `json:"entries"`
}

func LoadHistory(path string) *History {
	data, err := os.ReadFile(path)
	if err != nil {
		return &History{}
	}
	var history History
	if err := json.Unmarshal(data, &history); err == nil && history.Entries != nil {
		if len(history.Entries) > historyCap {
			history.Entries = history.Entries[:historyCap]
		}
		return &history
	}
	var entries []HistoryEntry
	if err := json.Unmarshal(data, &entries); err == nil {
		if len(entries) > historyCap {
			entries = entries[:historyCap]
		}
		return &History{Entries: entries}
	}
	return &History{}
}

func (h *History) Save(path string) error {
	directory := filepath.Dir(path)
	if err := os.MkdirAll(directory, 0o700); err != nil {
		return err
	}
	data, err := json.MarshalIndent(h, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	temporary, err := os.CreateTemp(directory, ".calculator-history-*.tmp")
	if err != nil {
		return err
	}
	name := temporary.Name()
	if _, err := temporary.Write(data); err != nil {
		temporary.Close()
		os.Remove(name)
		return err
	}
	if err := temporary.Sync(); err != nil {
		temporary.Close()
		os.Remove(name)
		return err
	}
	if err := temporary.Close(); err != nil {
		os.Remove(name)
		return err
	}
	if err := os.Rename(name, path); err != nil {
		os.Remove(name)
		return err
	}
	return nil
}

func (h *History) Record(expression, result string) {
	if len(h.Entries) > 0 {
		first := h.Entries[0]
		if first.Expression == expression && first.Result == result {
			return
		}
	}
	entry := HistoryEntry{Expression: expression, Result: result}
	h.Entries = append([]HistoryEntry{entry}, h.Entries...)
	if len(h.Entries) > historyCap {
		h.Entries = h.Entries[:historyCap]
	}
}

func (h *History) Search(query string) []HistoryEntry {
	query = strings.TrimSpace(query)
	if query == "" {
		return append([]HistoryEntry(nil), h.Entries...)
	}
	needle := strings.ToLower(query)
	var matches []HistoryEntry
	for _, entry := range h.Entries {
		if strings.Contains(strings.ToLower(entry.Expression), needle) ||
			strings.Contains(strings.ToLower(entry.Result), needle) {
			matches = append(matches, entry)
		}
	}
	return matches
}

// HistoryCopyPayload strips thousands grouping only from grouped numbers, not
// from dates such as "Friday, 14 August".
func HistoryCopyPayload(result string) string {
	trimmed := strings.TrimSpace(result)
	if !strings.Contains(trimmed, ",") {
		return result
	}
	if _, err := strconv.ParseFloat(strings.ReplaceAll(trimmed, ",", ""), 64); err == nil {
		return strings.ReplaceAll(trimmed, ",", "")
	}
	return result
}
