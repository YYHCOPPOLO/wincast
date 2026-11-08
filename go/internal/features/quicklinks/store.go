// Package quicklinks stores authored links and opens resolved destinations.
package quicklinks

import (
	"encoding/json"
	"errors"
	"io/fs"
	"log"
	"os"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/quicklink"
	"github.com/YYHCOPPOLO/wincast/go/internal/storage"
	"github.com/egoist/mygo"
)

// ErrUnavailable reports a library that could not be read. The store never
// replaces an unreadable file with an empty one.
var ErrUnavailable = errors.New("the quicklink library is unavailable")

// Store is the JSON quicklink library.
type Store struct {
	path      string
	links     []quicklink.Link
	available bool
	lastErr   error
}

func Load(path string) *Store {
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
	var links []quicklink.Link
	if err := json.Unmarshal(data, &links); err != nil {
		store.lastErr = err
		return store
	}
	store.links = normalize(links)
	store.available = true
	return store
}

func normalize(links []quicklink.Link) []quicklink.Link {
	clean := make([]quicklink.Link, 0, len(links))
	seen := make(map[string]bool, len(links))
	for _, link := range links {
		link.ID = strings.TrimSpace(link.ID)
		link.Name = strings.TrimSpace(link.Name)
		link.Destination = strings.TrimSpace(link.Destination)
		link.Hotkey = strings.TrimSpace(link.Hotkey)
		if link.ID == "" || link.Name == "" || link.Destination == "" || seen[link.ID] {
			continue
		}
		seen[link.ID] = true
		clean = append(clean, link)
	}
	quicklink.Sort(clean)
	return clean
}

func (s *Store) Available() bool { return s.available }

func (s *Store) LastError() error { return s.lastErr }

func (s *Store) Links() []quicklink.Link { return s.links }

func (s *Store) Get(id string) (quicklink.Link, bool) {
	for _, link := range s.links {
		if link.ID == id {
			return link, true
		}
	}
	return quicklink.Link{}, false
}

// Upsert creates or replaces a link, keeping its identity (and therefore its
// favorite slot and shortcut) while editing.
func (s *Store) Upsert(link quicklink.Link) (quicklink.Link, error) {
	if !s.available {
		return quicklink.Link{}, ErrUnavailable
	}
	link.Name = strings.TrimSpace(link.Name)
	link.Destination = strings.TrimSpace(link.Destination)
	link.Hotkey = strings.TrimSpace(link.Hotkey)
	if link.Name == "" || link.Destination == "" {
		return quicklink.Link{}, errors.New("a quicklink needs a name and a destination")
	}
	if link.ID == "" {
		id, err := storage.NewID()
		if err != nil {
			return quicklink.Link{}, err
		}
		link.ID = id
	}
	replaced := false
	next := make([]quicklink.Link, 0, len(s.links)+1)
	for _, existing := range s.links {
		if existing.ID == link.ID {
			next = append(next, link)
			replaced = true
			continue
		}
		next = append(next, existing)
	}
	if !replaced {
		link.PinOrder = nextPinOrder(next)
		next = append(next, link)
	}
	quicklink.Sort(next)
	if err := s.persist(next); err != nil {
		return quicklink.Link{}, err
	}
	s.links = next
	return link, nil
}

func nextPinOrder(links []quicklink.Link) uint64 {
	var maximum uint64
	for _, link := range links {
		if link.Pinned && link.PinOrder > maximum {
			maximum = link.PinOrder
		}
	}
	return maximum + 1
}

func (s *Store) Remove(id string) error {
	if !s.available {
		return ErrUnavailable
	}
	next := make([]quicklink.Link, 0, len(s.links))
	for _, link := range s.links {
		if link.ID == id {
			continue
		}
		next = append(next, link)
	}
	if err := s.persist(next); err != nil {
		return err
	}
	s.links = next
	return nil
}

// TogglePin pins or unpins a link. A newly pinned link goes last in the pin
// order so the established pins keep their positions.
func (s *Store) TogglePin(id string) (quicklink.Link, error) {
	link, ok := s.Get(id)
	if !ok {
		return quicklink.Link{}, errors.New("the quicklink no longer exists")
	}
	link.Pinned = !link.Pinned
	link.PinOrder = 0
	if link.Pinned {
		link.PinOrder = nextPinOrder(s.links)
	}
	return s.Upsert(link)
}

func (s *Store) persist(links []quicklink.Link) error {
	data, err := json.MarshalIndent(links, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	return storage.WriteFileAtomic(s.path, data, 0o600)
}

// Open resolves and opens a destination. Missing paths and failed opens are
// reported to the caller, which shows them in the UI.
func Open(destination string) error {
	kind, target := quicklink.Resolve(destination)
	if kind == quicklink.OpenPath {
		target = expandHome(target)
		info, err := os.Stat(target)
		if err != nil {
			return err
		}
		if !info.IsDir() && !info.Mode().IsRegular() {
			return errors.New("the destination is not a file or folder: " + target)
		}
		log.Printf("quicklink open path")
		return mygo.Shell.OpenPath(target)
	}
	if !strings.Contains(target, "://") && !strings.HasPrefix(target, "mailto:") {
		return mygo.Shell.OpenExternal("https://" + target)
	}
	log.Printf("quicklink open external")
	return mygo.Shell.OpenExternal(target)
}

func expandHome(path string) string {
	if !strings.HasPrefix(path, "~/") && path != "~" {
		return path
	}
	home, err := os.UserHomeDir()
	if err != nil {
		return path
	}
	if path == "~" {
		return home
	}
	return home + path[1:]
}
