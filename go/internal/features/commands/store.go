// Package commands stores user-defined shell commands and runs them through
// the platform runner. The store never deletes an unreadable library.
package commands

import (
	"encoding/json"
	"errors"
	"io/fs"
	"os"
	"sort"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
)

// ErrUnavailable reports a library that could not be read. Mutations refuse
// instead of replacing the user's file with defaults.
var ErrUnavailable = errors.New("the command library is unavailable")

// Command is one authored shell command.
type Command struct {
	ID      string `json:"id"`
	Name    string `json:"name"`
	Text    string `json:"command"`
	Confirm bool   `json:"confirm,omitempty"`
	Hotkey  string `json:"hotkey,omitempty"`
}

// EntryID is the launcher identity for a command.
func EntryID(id string) string { return "custom-command:" + strings.ToLower(id) }

// IDFromEntry returns the command id from a launcher identity.
func IDFromEntry(entryID string) (string, bool) {
	return strings.CutPrefix(entryID, "custom-command:")
}

// Sort orders commands case-insensitively by name, then by id.
func Sort(commands []Command) {
	sort.SliceStable(commands, func(i, j int) bool {
		left, right := strings.ToLower(commands[i].Name), strings.ToLower(commands[j].Name)
		if left != right {
			return left < right
		}
		return commands[i].ID < commands[j].ID
	})
}

// Store is the JSON library. The zero value is unusable; call Open.
type Store struct {
	path      string
	commands  []Command
	available bool
	lastErr   error
}

func Open(path string) *Store {
	store := &Store{path: path}
	data, err := os.ReadFile(path)
	if errors.Is(err, fs.ErrNotExist) {
		store.available = true
		return store
	}
	if err != nil {
		store.lastErr = err
		return store
	}
	var commands []Command
	if err := json.Unmarshal(data, &commands); err != nil {
		store.lastErr = err
		return store
	}
	store.commands = normalize(commands)
	store.available = true
	return store
}

func normalize(commands []Command) []Command {
	clean := make([]Command, 0, len(commands))
	seen := make(map[string]bool, len(commands))
	for _, command := range commands {
		command.ID = strings.TrimSpace(command.ID)
		command.Name = strings.TrimSpace(command.Name)
		command.Text = strings.TrimSpace(command.Text)
		command.Hotkey = strings.TrimSpace(command.Hotkey)
		if command.ID == "" || command.Name == "" || command.Text == "" || seen[strings.ToLower(command.ID)] {
			continue
		}
		seen[strings.ToLower(command.ID)] = true
		clean = append(clean, command)
	}
	Sort(clean)
	return clean
}

func (s *Store) Available() bool { return s.available }

func (s *Store) LastError() error { return s.lastErr }

func (s *Store) Commands() []Command { return s.commands }

func (s *Store) Get(id string) (Command, bool) {
	for _, command := range s.commands {
		if strings.EqualFold(command.ID, id) {
			return command, true
		}
	}
	return Command{}, false
}

// Upsert creates or replaces one command. A new command receives a UUID.
func (s *Store) Upsert(command Command) (Command, error) {
	if !s.available {
		return Command{}, ErrUnavailable
	}
	command.Name = strings.TrimSpace(command.Name)
	command.Text = strings.TrimSpace(command.Text)
	command.Hotkey = strings.TrimSpace(command.Hotkey)
	if command.Name == "" || command.Text == "" {
		return Command{}, errors.New("a command needs a name and a command line")
	}
	if command.ID == "" {
		id, err := storage.NewID()
		if err != nil {
			return Command{}, err
		}
		command.ID = id
	}
	replaced := false
	next := make([]Command, 0, len(s.commands)+1)
	for _, existing := range s.commands {
		if strings.EqualFold(existing.ID, command.ID) {
			next = append(next, command)
			replaced = true
			continue
		}
		next = append(next, existing)
	}
	if !replaced {
		next = append(next, command)
	}
	Sort(next)
	if err := s.persist(next); err != nil {
		return Command{}, err
	}
	s.commands = next
	return command, nil
}

// Remove deletes one command by identity.
func (s *Store) Remove(id string) error {
	if !s.available {
		return ErrUnavailable
	}
	next := make([]Command, 0, len(s.commands))
	for _, command := range s.commands {
		if strings.EqualFold(command.ID, id) {
			continue
		}
		next = append(next, command)
	}
	if err := s.persist(next); err != nil {
		return err
	}
	s.commands = next
	return nil
}

func (s *Store) persist(commands []Command) error {
	data, err := json.MarshalIndent(commands, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return storage.WriteFileAtomic(s.path, data, 0o600)
}
