package extensions

import (
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
)

// Installed is an extension's manifest and where it lives on disk.
type Installed struct {
	Manifest  *extension.Manifest
	Directory string
}

// ID is the manifest name, which namespaces storage and launcher entries.
func (i *Installed) ID() string { return i.Manifest.Name }

// Title is the human name.
func (i *Installed) Title() string { return i.Manifest.Title }

// AssetsPath is the directory extension artwork paths are relative to.
func (i *Installed) AssetsPath() string { return filepath.Join(i.Directory, "assets") }

// IconPath returns the manifest artwork whether it lives in assets/ or at the
// extension root.
func (i *Installed) IconPath() string {
	if i.Manifest.Icon == "" {
		return ""
	}
	for _, candidate := range []string{
		filepath.Join(i.Directory, "assets", i.Manifest.Icon),
		filepath.Join(i.Directory, i.Manifest.Icon),
	} {
		if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
			return candidate
		}
	}
	return ""
}

// CommandIconPath is a command's own artwork, falling back to the manifest.
func (i *Installed) CommandIconPath(command extension.Command) string {
	if command.Icon == "" {
		return ""
	}
	for _, candidate := range []string{
		filepath.Join(i.Directory, "assets", command.Icon),
		filepath.Join(i.Directory, command.Icon),
	} {
		if info, err := os.Stat(candidate); err == nil && !info.IsDir() {
			return candidate
		}
	}
	return ""
}

// Command finds a command by name.
func (i *Installed) Command(name string) (extension.Command, bool) {
	for _, command := range i.Manifest.Commands {
		if command.Name == name {
			return command, true
		}
	}
	return extension.Command{}, false
}

// BundlePath is the prebuilt CommonJS bundle for a command.
func (i *Installed) BundlePath(command extension.Command) (string, bool) {
	path := filepath.Join(i.Directory, command.Name+".js")
	if info, err := os.Stat(path); err == nil && !info.IsDir() {
		return path, true
	}
	return "", false
}

// CommandRef names one command of one extension.
type CommandRef struct {
	ExtensionName string
	CommandName   string
}

// EntryID is the launcher entry id Raycast-compatible extensions are surfaced
// under.
func (r CommandRef) EntryID() string {
	return "extension:" + r.ExtensionName + "/" + r.CommandName
}

// ParseEntryID reverses EntryID.
func ParseEntryID(entryID string) (CommandRef, bool) {
	if !strings.HasPrefix(entryID, "extension:") {
		return CommandRef{}, false
	}
	body := strings.TrimPrefix(entryID, "extension:")
	slash := strings.LastIndex(body, "/")
	if slash < 0 {
		return CommandRef{}, false
	}
	return CommandRef{ExtensionName: body[:slash], CommandName: body[slash+1:]}, true
}

// Catalog owns the extensions directory and install/uninstall.
type Catalog struct {
	root        string
	dataRoot    string
	supportRoot string
}

// NewCatalog resolves the extension layout under the application settings
// directory. The shape mirrors Raycast's own: one directory per extension,
// package.json, one <command>.js each, and assets/.
func NewCatalog(settingsDirectory string) *Catalog {
	return &Catalog{
		root:        filepath.Join(settingsDirectory, "extensions"),
		dataRoot:    filepath.Join(settingsDirectory, "extension-data"),
		supportRoot: filepath.Join(settingsDirectory, "extension-support"),
	}
}

// Root is the extensions directory.
func (c *Catalog) Root() string { return c.root }

// DataRoot is the per-extension storage directory.
func (c *Catalog) DataRoot() string { return c.dataRoot }

// SupportRoot holds each extension's environment.supportPath.
func (c *Catalog) SupportRoot() string { return c.supportRoot }

// SupportPath is one extension's scratch directory.
func (c *Catalog) SupportPath(name string) string {
	return filepath.Join(c.supportRoot, SafeName(name))
}

// EnsureSupportPath creates the extension's support directory.
func (c *Catalog) EnsureSupportPath(name string) string {
	path := c.SupportPath(name)
	_ = os.MkdirAll(path, 0o700)
	return path
}

// Scan lists installed extensions. An unreadable or half-written directory is
// skipped rather than failing the whole scan.
func (c *Catalog) Scan() []*Installed {
	entries, err := os.ReadDir(c.root)
	if err != nil {
		return nil
	}
	installed := make([]*Installed, 0, len(entries))
	for _, entry := range entries {
		if !entry.IsDir() || strings.HasPrefix(entry.Name(), ".") {
			continue
		}
		directory := filepath.Join(c.root, entry.Name())
		manifest, err := extension.LoadManifest(directory)
		if err != nil {
			continue
		}
		installed = append(installed, &Installed{Manifest: manifest, Directory: directory})
	}
	sort.SliceStable(installed, func(i, j int) bool {
		return strings.ToLower(installed[i].Title()) < strings.ToLower(installed[j].Title())
	})
	return installed
}

// InstallError explains why a folder could not become an extension.
type InstallError struct {
	Message string
}

func (e *InstallError) Error() string { return e.Message }

// Install copies only the manifest, built command bundles and assets, never
// node_modules or source maps. The source may be a folder the user picked or a
// folder unpacked from the Raycast store.
func (c *Catalog) Install(source string) (*Installed, error) {
	manifest, err := extension.LoadManifest(source)
	if err != nil {
		return nil, &InstallError{Message: fmt.Sprintf("%s doesn't contain a Raycast extension (no package.json with commands).", filepath.Base(source))}
	}
	if !manifest.SupportsWindows() {
		return nil, &InstallError{Message: fmt.Sprintf("%s doesn't support Windows.", manifest.Title)}
	}
	built := make([]extension.Command, 0, len(manifest.Commands))
	for _, command := range manifest.Commands {
		path := filepath.Join(source, command.Name+".js")
		if info, err := os.Stat(path); err == nil && !info.IsDir() {
			built = append(built, command)
		}
	}
	if len(built) == 0 {
		return nil, &InstallError{Message: fmt.Sprintf("%s has no built command bundles. Wincast installs prebuilt extensions — run `ray build` in the extension folder first, or import one from an installed Raycast.", manifest.Title)}
	}

	destination := filepath.Join(c.root, strings.ReplaceAll(manifest.Name, "/", "-"))
	if err := os.RemoveAll(destination); err != nil {
		return nil, &InstallError{Message: fmt.Sprintf("Couldn't replace the existing extension: %v", err)}
	}
	if err := os.MkdirAll(destination, 0o700); err != nil {
		return nil, &InstallError{Message: fmt.Sprintf("Couldn't install the extension: %v", err)}
	}
	if err := copyFile(filepath.Join(source, "package.json"), filepath.Join(destination, "package.json")); err != nil {
		return nil, &InstallError{Message: fmt.Sprintf("Couldn't install the extension: %v", err)}
	}
	for _, command := range built {
		name := command.Name + ".js"
		if err := copyFile(filepath.Join(source, name), filepath.Join(destination, name)); err != nil {
			return nil, &InstallError{Message: fmt.Sprintf("Couldn't install the extension: %v", err)}
		}
	}
	assets := filepath.Join(source, "assets")
	if info, err := os.Stat(assets); err == nil && info.IsDir() {
		if err := copyTree(assets, filepath.Join(destination, "assets")); err != nil {
			return nil, &InstallError{Message: fmt.Sprintf("Couldn't install the extension: %v", err)}
		}
	}

	installedManifest, err := extension.LoadManifest(destination)
	if err != nil {
		return nil, &InstallError{Message: fmt.Sprintf("Couldn't re-read the installed extension: %v", err)}
	}
	return &Installed{Manifest: installedManifest, Directory: destination}, nil
}

// Uninstall removes the bundle and the extension's own scratch directory. The
// manager also clears storage, shortcuts and launcher references.
func (c *Catalog) Uninstall(installed *Installed) error {
	_ = os.RemoveAll(c.SupportPath(installed.Manifest.Name))
	return os.RemoveAll(installed.Directory)
}

// RaycastRoots are the local Raycast extension directories checked by the
// import route. Raycast for Windows keeps its data under the roaming profile;
// both channels and a few historical layouts are checked so an installation
// is found wherever the app put it.
func RaycastRoots() []string {
	var roots []string
	if config, err := os.UserConfigDir(); err == nil {
		roots = append(roots,
			filepath.Join(config, "Raycast", "extensions"),
			filepath.Join(config, "Raycast", "data", "extensions"),
			filepath.Join(config, "raycast", "extensions"),
			filepath.Join(config, "raycast-x", "extensions"),
		)
	}
	if local, err := os.UserCacheDir(); err == nil {
		roots = append(roots,
			filepath.Join(local, "Raycast", "extensions"),
			filepath.Join(local, "Raycast", "data", "extensions"),
			filepath.Join(local, "raycast", "extensions"),
		)
	}
	if home, err := os.UserHomeDir(); err == nil {
		roots = append(roots, filepath.Join(home, ".config", "raycast", "extensions"))
	}
	return roots
}

// RaycastDirectory returns the first Raycast root holding an extension, so an
// empty sibling directory never reads as "Raycast is not installed".
func RaycastDirectory() string {
	for _, root := range RaycastRoots() {
		entries, err := os.ReadDir(root)
		if err != nil {
			continue
		}
		for _, entry := range entries {
			if !entry.IsDir() || strings.HasPrefix(entry.Name(), ".") {
				continue
			}
			if _, err := os.Stat(filepath.Join(root, entry.Name(), "package.json")); err == nil {
				return root
			}
		}
	}
	return ""
}

// ImportRaycast copies every installed Raycast extension Wincast does not have
// and returns the imported manifests.
func (c *Catalog) ImportRaycast() ([]*Installed, []string) {
	root := RaycastDirectory()
	if root == "" {
		return nil, nil
	}
	entries, err := os.ReadDir(root)
	if err != nil {
		return nil, nil
	}
	have := make(map[string]bool)
	for _, installed := range c.Scan() {
		have[installed.Manifest.Name] = true
	}
	var imported []*Installed
	var failures []string
	for _, entry := range entries {
		if !entry.IsDir() || strings.HasPrefix(entry.Name(), ".") {
			continue
		}
		source := filepath.Join(root, entry.Name())
		manifest, err := extension.LoadManifest(source)
		if err != nil || have[manifest.Name] {
			continue
		}
		installed, err := c.Install(source)
		if err != nil {
			failures = append(failures, err.Error())
			continue
		}
		have[installed.Manifest.Name] = true
		imported = append(imported, installed)
	}
	return imported, failures
}

func copyFile(source, destination string) error {
	in, err := os.Open(source)
	if err != nil {
		return err
	}
	defer in.Close()
	if err := os.MkdirAll(filepath.Dir(destination), 0o700); err != nil {
		return err
	}
	out, err := os.Create(destination)
	if err != nil {
		return err
	}
	if _, err := io.Copy(out, in); err != nil {
		out.Close()
		return err
	}
	return out.Close()
}

func copyTree(source, destination string) error {
	return filepath.WalkDir(source, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		relative, err := filepath.Rel(source, path)
		if err != nil {
			return err
		}
		target := filepath.Join(destination, relative)
		if entry.IsDir() {
			return os.MkdirAll(target, 0o700)
		}
		if !entry.Type().IsRegular() {
			return nil
		}
		return copyFile(path, target)
	})
}

var errNotAnExtension = errors.New("not a Raycast extension")
