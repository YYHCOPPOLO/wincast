// Package extension holds the data the Raycast extension runtime and the
// palette share: the manifest, the render tree and the value types that
// cross the JavaScript bridge. Nothing here touches the disk or the UI.
package extension

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strconv"
	"strings"
)

// CommandMode is how a Raycast command presents itself.
type CommandMode string

const (
	ModeView    CommandMode = "view"
	ModeNoView  CommandMode = "no-view"
	ModeMenuBar CommandMode = "menu-bar"
)

// RuntimeName is the string the JavaScript runtime understands; it only
// distinguishes mounted from headless.
func (m CommandMode) RuntimeName() string {
	if m == ModeView {
		return "view"
	}
	return "no-view"
}

// Supported reports whether Wincast can run this command. Menu-bar commands
// are recognised so the launcher can label them, but the palette is not a
// menu bar host.
func (m CommandMode) Supported() bool { return m != ModeMenuBar }

// UnsupportedReason is the sentence shown when a command cannot run.
func (m CommandMode) UnsupportedReason() string {
	if m == ModeMenuBar {
		return "Menu bar commands aren't supported yet — Wincast only runs view and no-view commands."
	}
	return ""
}

// PreferenceKind is one preference editor control.
type PreferenceKind string

const (
	PreferenceText      PreferenceKind = "textfield"
	PreferencePassword  PreferenceKind = "password"
	PreferenceCheckbox  PreferenceKind = "checkbox"
	PreferenceDropdown  PreferenceKind = "dropdown"
	PreferenceAppPicker PreferenceKind = "appPicker"
	PreferenceFile      PreferenceKind = "file"
	PreferenceDirectory PreferenceKind = "directory"
)

// PreferenceOption is one choice of a dropdown preference.
type PreferenceOption struct {
	Title string
	Value string
}

// Preference is one entry of a manifest's preferences array.
type Preference struct {
	Name        string
	Title       string
	Label       string
	Description string
	Placeholder string
	Kind        PreferenceKind
	Required    bool
	Options     []PreferenceOption
	Default     *PreferenceValue
}

// DisplayTitle is the label the settings editor uses.
func (p Preference) DisplayTitle() string {
	if p.Title != "" {
		return p.Title
	}
	if p.Label != "" {
		return p.Label
	}
	return p.Name
}

// EffectiveDefault is the value used when the user has not set one.
func (p Preference) EffectiveDefault() PreferenceValue {
	if p.Default != nil {
		return *p.Default
	}
	if p.Kind == PreferenceCheckbox {
		return PreferenceValue{Kind: ValueBool}
	}
	return PreferenceValue{Kind: ValueString}
}

// Argument is one command argument collected in the palette search field row.
type Argument struct {
	Name        string
	Type        string
	Placeholder string
	Required    bool
}

// Command is one manifest command.
type Command struct {
	Name              string
	Title             string
	Subtitle          string
	Description       string
	Mode              CommandMode
	Keywords          []string
	Icon              string
	DisabledByDefault bool
	Preferences       []Preference
	Arguments         []Argument
}

// CompleteArguments returns every declared argument, empty when the user
// left it blank. Extensions depend on this: Number("") is 0, Number(undefined)
// is NaN, and a missing blank argument silently corrupts their arithmetic.
func (c Command) CompleteArguments(given map[string]string) map[string]string {
	complete := make(map[string]string, len(c.Arguments))
	for key, value := range given {
		complete[key] = value
	}
	for _, argument := range c.Arguments {
		if _, ok := complete[argument.Name]; !ok {
			complete[argument.Name] = ""
		}
	}
	return complete
}

// Manifest is a Raycast package.json reduced to what the runtime uses.
type Manifest struct {
	Name        string
	Title       string
	Description string
	Author      string
	Icon        string
	Categories  []string
	Platforms   []string
	Commands    []Command
	Preferences []Preference
}

// SupportsWindows reports whether the manifest lists Windows. An absent
// platforms array is accepted: most shipped manifests predate Raycast's
// Windows build and are still plain JavaScript.
func (m Manifest) SupportsWindows() bool {
	if len(m.Platforms) == 0 {
		return true
	}
	for _, platform := range m.Platforms {
		if strings.EqualFold(platform, "Windows") {
			return true
		}
	}
	return false
}

// ParseManifest decodes a package.json. A manifest without a name or any
// command is not an extension.
func ParseManifest(data []byte) (*Manifest, error) {
	var raw struct {
		Name        string            `json:"name"`
		Title       string            `json:"title"`
		Description string            `json:"description"`
		Author      json.RawMessage   `json:"author"`
		Icon        string            `json:"icon"`
		Categories  []string          `json:"categories"`
		Platforms   []string          `json:"platforms"`
		Commands    []json.RawMessage `json:"commands"`
		Preferences []json.RawMessage `json:"preferences"`
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	if err := decoder.Decode(&raw); err != nil {
		return nil, fmt.Errorf("read package.json: %w", err)
	}
	if raw.Name == "" {
		return nil, fmt.Errorf("package.json has no name")
	}
	manifest := &Manifest{
		Name:        raw.Name,
		Title:       raw.Title,
		Description: raw.Description,
		Author:      authorName(raw.Author),
		Icon:        raw.Icon,
		Categories:  raw.Categories,
		Platforms:   raw.Platforms,
	}
	if manifest.Title == "" {
		manifest.Title = manifest.Name
	}
	for _, entry := range raw.Commands {
		if command, ok := parseCommand(entry); ok {
			manifest.Commands = append(manifest.Commands, command)
		}
	}
	if len(manifest.Commands) == 0 {
		return nil, fmt.Errorf("%s has no commands", raw.Name)
	}
	for _, entry := range raw.Preferences {
		if preference, ok := parsePreference(entry); ok {
			manifest.Preferences = append(manifest.Preferences, preference)
		}
	}
	return manifest, nil
}

// LoadManifest reads a directory's package.json.
func LoadManifest(directory string) (*Manifest, error) {
	data, err := os.ReadFile(filepath.Join(directory, "package.json"))
	if err != nil {
		return nil, fmt.Errorf("read manifest: %w", err)
	}
	return ParseManifest(data)
}

func authorName(raw json.RawMessage) string {
	if len(raw) == 0 {
		return ""
	}
	var text string
	if json.Unmarshal(raw, &text) == nil {
		return text
	}
	var object struct {
		Name   string `json:"name"`
		Handle string `json:"handle"`
	}
	if json.Unmarshal(raw, &object) == nil {
		if object.Name != "" {
			return object.Name
		}
		return object.Handle
	}
	return ""
}

func parseCommand(raw json.RawMessage) (Command, bool) {
	var entry struct {
		Name              string            `json:"name"`
		Title             string            `json:"title"`
		Subtitle          string            `json:"subtitle"`
		Description       string            `json:"description"`
		Mode              string            `json:"mode"`
		Keywords          []string          `json:"keywords"`
		Icon              string            `json:"icon"`
		DisabledByDefault bool              `json:"disabledByDefault"`
		Preferences       []json.RawMessage `json:"preferences"`
		Arguments         []json.RawMessage `json:"arguments"`
	}
	if json.Unmarshal(raw, &entry) != nil || entry.Name == "" || entry.Title == "" {
		return Command{}, false
	}
	mode := CommandMode(entry.Mode)
	switch mode {
	case ModeView, ModeNoView, ModeMenuBar:
	default:
		if entry.Mode == "" {
			mode = ModeView
		} else {
			return Command{}, false
		}
	}
	command := Command{
		Name:              entry.Name,
		Title:             entry.Title,
		Subtitle:          entry.Subtitle,
		Description:       entry.Description,
		Mode:              mode,
		Keywords:          entry.Keywords,
		Icon:              entry.Icon,
		DisabledByDefault: entry.DisabledByDefault,
	}
	for _, item := range entry.Preferences {
		if preference, ok := parsePreference(item); ok {
			command.Preferences = append(command.Preferences, preference)
		}
	}
	for _, item := range entry.Arguments {
		var argument struct {
			Name        string `json:"name"`
			Type        string `json:"type"`
			Placeholder string `json:"placeholder"`
			Required    bool   `json:"required"`
		}
		if json.Unmarshal(item, &argument) != nil || argument.Name == "" {
			continue
		}
		if argument.Type == "" {
			argument.Type = "text"
		}
		if argument.Placeholder == "" {
			argument.Placeholder = argument.Name
		}
		command.Arguments = append(command.Arguments, Argument{
			Name: argument.Name, Type: argument.Type,
			Placeholder: argument.Placeholder, Required: argument.Required,
		})
	}
	return command, true
}

func parsePreference(raw json.RawMessage) (Preference, bool) {
	var entry struct {
		Name        string `json:"name"`
		Title       string `json:"title"`
		Label       string `json:"label"`
		Description string `json:"description"`
		Placeholder string `json:"placeholder"`
		Type        string `json:"type"`
		Required    bool   `json:"required"`
		Data        []struct {
			Title string `json:"title"`
			Value string `json:"value"`
		} `json:"data"`
		Default json.RawMessage `json:"default"`
	}
	if json.Unmarshal(raw, &entry) != nil || entry.Name == "" {
		return Preference{}, false
	}
	kind := PreferenceKind(entry.Type)
	switch kind {
	case PreferenceText, PreferencePassword, PreferenceCheckbox, PreferenceDropdown,
		PreferenceAppPicker, PreferenceFile, PreferenceDirectory:
	default:
		kind = PreferenceText
	}
	preference := Preference{
		Name: entry.Name, Title: entry.Title, Label: entry.Label,
		Description: entry.Description, Placeholder: entry.Placeholder,
		Kind: kind, Required: entry.Required,
	}
	for _, option := range entry.Data {
		if option.Value == "" {
			continue
		}
		title := option.Title
		if title == "" {
			title = option.Value
		}
		preference.Options = append(preference.Options, PreferenceOption{Title: title, Value: option.Value})
	}
	if value, ok := parseDefault(entry.Default); ok {
		preference.Default = &value
	}
	return preference, true
}

// parseDefault accepts a scalar or a platform-keyed object. Windows wins;
// macOS is the fallback so a macOS-only default still gives a useful value.
func parseDefault(raw json.RawMessage) (PreferenceValue, bool) {
	if len(raw) == 0 || bytes.Equal(raw, []byte("null")) {
		return PreferenceValue{}, false
	}
	var decoded any
	if json.Unmarshal(raw, &decoded) == nil {
		switch typed := decoded.(type) {
		case string:
			return PreferenceValue{Kind: ValueString, String: typed}, true
		case float64:
			return PreferenceValue{Kind: ValueNumber, Number: typed}, true
		case bool:
			return PreferenceValue{Kind: ValueBool, Bool: typed}, true
		case map[string]any:
			for _, key := range []string{"Windows", "macOS", "windows", "macos"} {
				if nested, ok := typed[key]; ok {
					switch item := nested.(type) {
					case string:
						return PreferenceValue{Kind: ValueString, String: item}, true
					case float64:
						return PreferenceValue{Kind: ValueNumber, Number: item}, true
					case bool:
						return PreferenceValue{Kind: ValueBool, Bool: item}, true
					}
				}
			}
		}
	}
	return PreferenceValue{}, false
}

// ValueKind is the JavaScript shape of a stored scalar.
type ValueKind uint8

const (
	ValueNull ValueKind = iota
	ValueString
	ValueNumber
	ValueBool
	ValueDate
	ValueHandler
	ValueArray
	ValueObject
	ValueNode
)

// PreferenceValue is a manifest default or a user-set preference.
type PreferenceValue struct {
	Kind   ValueKind
	String string
	Number float64
	Bool   bool
}

// FromJSON converts a decoded JSON scalar.
func FromJSON(value any) (PreferenceValue, bool) {
	switch typed := value.(type) {
	case string:
		return PreferenceValue{Kind: ValueString, String: typed}, true
	case float64:
		return PreferenceValue{Kind: ValueNumber, Number: typed}, true
	case bool:
		return PreferenceValue{Kind: ValueBool, Bool: typed}, true
	default:
		return PreferenceValue{}, false
	}
}

// Any returns the value in the shape encoding/json writes.
func (v PreferenceValue) Any() any {
	switch v.Kind {
	case ValueString:
		return v.String
	case ValueNumber:
		return v.Number
	case ValueBool:
		return v.Bool
	default:
		return nil
	}
}

// StringValue renders a scalar the way the runtime's stringValue does.
func (v PreferenceValue) StringValue() string {
	switch v.Kind {
	case ValueString:
		return v.String
	case ValueNumber:
		if v.Number == float64(int64(v.Number)) && v.Number < 1e15 && v.Number > -1e15 {
			return strconv.FormatInt(int64(v.Number), 10)
		}
		return strconv.FormatFloat(v.Number, 'f', -1, 64)
	case ValueBool:
		if v.Bool {
			return "true"
		}
		return "false"
	default:
		return ""
	}
}

// BoolValue is the truthiness the preference editors need.
func (v PreferenceValue) BoolValue() bool {
	switch v.Kind {
	case ValueBool:
		return v.Bool
	case ValueNumber:
		return v.Number != 0
	case ValueString:
		return v.String == "true"
	default:
		return false
	}
}

// SortCommands orders commands by title, then name, as the settings pane does.
func SortCommands(commands []Command) {
	sort.SliceStable(commands, func(i, j int) bool {
		left, right := strings.ToLower(commands[i].Title), strings.ToLower(commands[j].Title)
		if left != right {
			return left < right
		}
		return commands[i].Name < commands[j].Name
	})
}
