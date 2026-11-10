package app

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
	"github.com/YYHCOPPOLO/wincast/go/internal/domain/template"
	"github.com/YYHCOPPOLO/wincast/go/internal/features/extensions"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/palette"
	"github.com/egoist/mygo"
	"github.com/egoist/mygo/ui"
)

// updateSettings posts a settings-window rebuild only while that window exists;
// installs run on a worker and may outlive the pane.
func (c *controller) updateSettings(fn func()) {
	if c.settings != nil {
		c.settings.Update(fn)
	}
}

// startExtensions builds the extension subsystem. The manager only scans while
// the user has turned extensions on; its events marshal onto the palette.
func (c *controller) startExtensions() {
	events := extensions.Events{
		Render: func(session string, tree *extension.Tree) {
			if c.panel == nil {
				return
			}
			c.panel.Update(func() {
				if c.closing || c.extensions == nil || c.extensions.Active() != session {
					return
				}
				if c.extensionSession == "" {
					c.extensionSession = session
					c.view.ExtensionSession = session
				}
				c.view.SetExtensionTree(tree)
			})
		},
		Failed: func(session, message string) {
			log.Printf("extension failed session=%s: %s", session, message)
			if c.panel == nil {
				return
			}
			c.panel.Update(func() {
				if c.closing || (c.extensionSession != "" && c.extensionSession != session) {
					return
				}
				if c.extensionMode == extension.ModeNoView {
					c.view.PendingToast = message
					c.hide()
					c.extensionEntryID = ""
					return
				}
				c.view.SetExtensionError(message)
			})
		},
		Finished: func(session string) {
			if c.panel == nil {
				return
			}
			c.panel.Update(func() {
				if c.closing || (c.extensionSession != "" && c.extensionSession != session) {
					return
				}
				if c.extensionMode == extension.ModeNoView {
					c.view.LeaveExtension()
					c.hide()
					c.extensionEntryID = ""
				}
			})
		},
		Started: func(session string) {
			log.Printf("extension started session=%s", session)
			if c.panel == nil {
				return
			}
			c.panel.Update(func() {
				if c.closing {
					return
				}
				c.extensionSession = session
				c.view.ExtensionSession = session
			})
		},
		NavigationDepth: func(session string, depth int) {
			if c.panel == nil {
				return
			}
			c.panel.Update(func() {
				if c.closing || (c.extensionSession != "" && c.extensionSession != session) {
					return
				}
				c.view.ExtensionLoading = false
			})
		},
		Log: func(level, message string) {
			log.Printf("extension %s: %s", level, message)
		},
		DidUninstall: func(extensionName string, entryIDs []string) {
			_ = os.RemoveAll(filepath.Join(c.profile.settings, "extension-oauth", extensions.SafeName(extensionName)))
			removed := false
			for _, entryID := range entryIDs {
				if _, ok := c.extensionHotkeys[entryID]; ok {
					delete(c.extensionHotkeys, entryID)
					removed = true
				}
			}
			if removed && c.settingsDocument != nil && c.settingsWritable {
				_ = c.settingsDocument.Set("extensionHotkeys", c.extensionHotkeys)
				c.queueSettingsSave()
			}
			if c.panel == nil {
				return
			}
			c.panel.Update(func() {
				if c.launcherPrefs != nil {
					for _, entryID := range entryIDs {
						_ = c.launcherPrefs.SetAlias(entryID, "")
						_ = c.launcherPrefs.SetHidden(entryID, false)
						if c.launcherPrefs.IsFavorite(entryID) {
							_ = c.launcherPrefs.ToggleFavorite(entryID)
						}
					}
				}
				c.refreshExtras()
				c.reconcileItemHotkeys()
			})
			_ = extensionName
		},
	}
	c.extensions = extensions.NewManager(c.profile.settings, c, events)
	c.extensions.SetShowInLauncher(c.extensionsShowInLauncher)
	c.extensions.SetAppearanceDark(c.darkAppearance())
	c.extensions.SetEnabled(c.extensionsEnabled)
}

func (c *controller) darkAppearance() bool {
	switch c.appearance {
	case "dark":
		return true
	case "light":
		return false
	}
	return windows.SystemDark()
}

// refreshExtensionExtras adds one launcher row per installed command.
func (c *controller) refreshExtensionExtras(extras []domain.Application) []domain.Application {
	if c.extensions == nil || !c.extensions.Enabled() || !c.extensions.ShowInLauncher() {
		return extras
	}
	for _, entry := range c.extensions.Commands() {
		iconPath := entry.Extension.CommandIconPath(entry.Command)
		if iconPath == "" {
			iconPath = entry.Extension.IconPath()
		}
		row := domain.Application{
			ID: entry.EntryID, Name: entry.Command.Title, Kind: domain.KindExtension, IconPath: iconPath,
		}
		// Raycast ranks a command by its extension too: typing "coffee" must
		// find Caffeinate, Caffeinate While and the rest.
		alternates := make([]string, 0, 4+len(entry.Command.Keywords))
		alternates = append(alternates, entry.Extension.Title())
		if entry.Command.Subtitle != "" {
			alternates = append(alternates, entry.Command.Subtitle)
		}
		if entry.Command.Description != "" {
			alternates = append(alternates, entry.Command.Description)
		}
		alternates = append(alternates, entry.Command.Keywords...)
		row.AlternateNames = alternates
		if appearance := c.extensionAppearanceFor(entry.Extension.ID()); appearance != nil {
			row.IconPath = ""
			row.IconSymbol = appearance.Symbol
			row.IconTint = appearance.Tint
		}
		extras = append(extras, row)
	}
	return extras
}

func (c *controller) startExtension(entryID string) {
	c.startExtensionWithArguments(entryID, nil)
}

func (c *controller) submitExtensionArguments(application domain.Application, values []string) {
	if c.extensions == nil {
		return
	}
	_, command, ok := c.extensions.Resolve(application.ID)
	if !ok {
		return
	}
	arguments := make(map[string]string, len(command.Arguments))
	for index, argument := range command.Arguments {
		if index < len(values) {
			arguments[argument.Name] = values[index]
		}
	}
	c.startExtensionWithArguments(application.ID, arguments)
}

func (c *controller) startExtensionWithArguments(entryID string, arguments map[string]string) {
	if c.extensions == nil || !c.extensions.Enabled() {
		return
	}
	owner, command, ok := c.extensions.Resolve(entryID)
	if !ok {
		c.view.Status = c.view.Text("扩展命令不存在", "That extension command is not installed")
		c.panel.Update(func() {})
		return
	}
	if c.panel == nil {
		return
	}
	if arguments == nil && len(command.Arguments) > 0 {
		specs := make([]template.Argument, 0, len(command.Arguments))
		for _, argument := range command.Arguments {
			specs = append(specs, template.Argument{Name: argument.Placeholder})
		}
		app := domain.Application{ID: entryID, Name: command.Title, Kind: domain.KindExtension}
		if c.view.SetModeArguments(app, specs) {
			c.view.Argument.Extension = true
			if !c.panel.IsVisible() {
				c.showPanel(false)
			}
			c.panel.Update(func() {})
		}
		return
	}
	c.extensionMode = command.Mode
	if command.Mode == extension.ModeNoView {
		c.hide()
		session, err := c.extensions.Start(entryID, arguments)
		if err != nil {
			c.view.PendingToast = err.Error()
			c.showPanel(false)
			return
		}
		c.extensionSession = session
		c.extensionEntryID = entryID
		return
	}
	c.view.EnsureExtension()
	c.view.ExtensionSession = ""
	c.view.ExtensionAssets = owner.AssetsPath()
	c.view.ExtensionTitle = command.Title
	c.view.SetExtensionLoading(command.Title)
	if c.panel != nil && !c.panel.IsVisible() {
		c.showPanel(false)
	}
	c.panel.Update(func() {})
	session, err := c.extensions.Start(entryID, arguments)
	if err != nil {
		c.extensionSession = ""
		c.extensionEntryID = ""
		c.view.SetExtensionError(err.Error())
		c.panel.Update(func() {})
		return
	}
	c.extensionSession = session
	c.extensionEntryID = entryID
	c.view.ExtensionSession = session
	c.panel.Update(func() {})
}

func (c *controller) dispatchExtension(session, handler, argsJSON string) {
	if c.extensions == nil || session == "" || session != c.extensionSession {
		return
	}
	if !c.extensions.Dispatch(session, handler, argsJSON) {
		log.Printf("extension dispatch failed session=%s handler=%s", session, handler)
	}
}

func (c *controller) popExtension(session string) bool {
	if c.extensions == nil || session != c.extensionSession {
		return false
	}
	return c.extensions.PopNavigation(session)
}

func (c *controller) closeExtension() {
	if c.extensions != nil {
		c.extensions.Stop()
	}
	c.extensionSession = ""
	c.extensionEntryID = ""
	c.view.LeaveExtension()
	c.hide()
}

func (c *controller) leaveExtension() {
	if c.extensions != nil {
		c.extensions.Stop()
	}
	c.extensionSession = ""
	c.extensionEntryID = ""
	c.view.LeaveExtension()
	c.panel.Update(func() {})
}

// Perform answers the capabilities the runtime asks the application for.
func (c *controller) Perform(api, method string, arguments []extension.Value) (any, error) {
	switch api {
	case "clipboard":
		return c.extensionClipboard(method, arguments)
	case "open":
		return c.extensionOpenTarget(method, arguments)
	case "finder":
		if method == "show" {
			path := extensionString(arguments, 0, "path")
			if path == "" {
				return nil, errors.New("showInFinder needs a path")
			}
			mygo.Shell.ShowItemInFolder(path)
			return nil, nil
		}
	case "trash":
		paths := extensionStringArray(arguments, 0, "paths")
		for _, path := range paths {
			if err := mygo.Shell.TrashItem(path); err != nil {
				return nil, err
			}
		}
		return nil, nil
	case "selection":
		if method == "text" {
			return c.extensionSelectedText()
		}
		if method == "files" {
			paths := windows.ClipboardFilePaths()
			files := make([]any, 0, len(paths))
			for _, path := range paths {
				files = append(files, map[string]any{"path": path})
			}
			return files, nil
		}
	case "applications":
		return c.extensionApplications(method, arguments)
	case "preferences":
		name := ""
		command := ""
		if c.extensions != nil {
			name = c.extensions.ActiveExtensionName()
			if extensionString(arguments, 0, "scope") == "command" {
				command = c.extensions.ActiveCommandName()
			}
		}
		c.openExtensionPreferences(name, command)
		return nil, nil
	case "command":
		if method == "launch" {
			return nil, c.extensionLaunchCommand(arguments)
		}
		if method == "metadata" {
			return nil, nil
		}
	case "oauth":
		return c.extensionOAuth(method, arguments)
	case "window":
		return c.extensionWindow(method, arguments)
	case "snippet", "quicklink", "quicklook", "mcp", "ai", "browser":
		return nil, fmt.Errorf("%s.%s is not supported in Wincast extensions.", api, method)
	}
	return nil, fmt.Errorf("Unknown host call '%s.%s'.", api, method)
}

func (c *controller) extensionClipboard(method string, arguments []extension.Value) (any, error) {
	switch method {
	case "copy", "paste":
		object := extensionObject(arguments, 0)
		text := ""
		if object != nil {
			if value, ok := object["text"].StringValue(); ok {
				text = value
			} else if value, ok := object["html"].StringValue(); ok {
				text = value
			}
		}
		if text == "" {
			return nil, nil
		}
		c.copyText(text, method == "paste")
		return nil, nil
	case "clear":
		_ = windows.WriteClipboardText(0, "", false)
		return nil, nil
	case "readText":
		text, _ := windows.ReadClipboardText()
		return text, nil
	case "read":
		payload := map[string]any{}
		if text, ok := windows.ReadClipboardText(); ok {
			payload["text"] = text
		}
		if files := windows.ClipboardFilePaths(); len(files) > 0 {
			payload["file"] = files[0]
		}
		return payload, nil
	}
	return nil, fmt.Errorf("Unknown clipboard call %s.", method)
}

func (c *controller) extensionOpenTarget(method string, arguments []extension.Value) (any, error) {
	object := extensionObject(arguments, 0)
	target := ""
	if object != nil {
		if method == "url" {
			target, _ = object["url"].StringValue()
		} else {
			target, _ = object["path"].StringValue()
		}
	}
	if target == "" {
		return nil, errors.New("open needs a target")
	}
	if method == "url" {
		if err := mygo.Shell.OpenExternal(target); err != nil {
			return nil, err
		}
		return nil, nil
	}
	if err := mygo.Shell.OpenPath(target); err != nil {
		return nil, err
	}
	return nil, nil
}

// pickExtensionFiles opens the native picker for Form.FilePicker. The host
// owns the dialog; the palette only reports the chosen paths.
func (c *controller) pickExtensionFiles(multiple, directories, files bool) []string {
	options := mygo.OpenDialogOptions{
		Parent:    c.panel,
		Title:     c.text("选择文件", "Choose Files"),
		Directory: directories,
		Multiple:  multiple,
	}
	paths, err := mygo.Dialog.Open(options)
	if err != nil || len(paths) == 0 {
		return nil
	}
	if !directories && !files {
		return nil
	}
	return paths
}

func (c *controller) extensionSelectedText() (string, error) {
	if c.target.Window == 0 {
		return "", errors.New("No application was in front of Wincast")
	}
	if c.panel == nil {
		return "", errors.New("The palette is unavailable")
	}
	done := make(chan struct{})
	c.panel.Update(func() {
		c.hide()
		close(done)
	})
	<-done
	c.clipboardSuppress.Store(true)
	time.Sleep(60 * time.Millisecond)
	ctx, cancel := context.WithTimeout(c.ctx, time.Second)
	text, err := windows.CaptureSelection(ctx, c.target, c.panel.NativeHandle())
	cancel()
	c.clipboardSuppress.Store(false)
	c.panel.Update(func() {
		if !c.closing && c.view.Mode == palette.ModeExtension {
			c.showPanel(false)
		}
	})
	if err != nil {
		return "", err
	}
	return text, nil
}

func (c *controller) extensionApplications(method string, arguments []extension.Value) (any, error) {
	describe := func(application domain.Application) map[string]any {
		return map[string]any{
			"name":          application.Name,
			"path":          application.Path,
			"bundleId":      application.ID,
			"localizedName": application.Name,
		}
	}
	switch method {
	case "frontmost":
		target := c.target
		if target.Window == 0 {
			target = windows.ForegroundTarget()
		}
		if target.Window == 0 {
			return nil, errors.New("No foreground application")
		}
		path := windows.ProcessImagePath(target.Process)
		name := ""
		if path != "" {
			name = strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
		}
		return map[string]any{
			"name": name, "path": path, "bundleId": nil, "localizedName": name,
		}, nil
	case "list":
		path := extensionString(arguments, 0, "path")
		applications := c.launcher.Applications()
		result := make([]any, 0, 8)
		for _, application := range applications {
			if path != "" && !strings.EqualFold(filepath.Clean(application.Path), filepath.Clean(path)) {
				continue
			}
			result = append(result, describe(application))
			if path == "" && len(result) >= 200 {
				break
			}
		}
		return result, nil
	case "default":
		path := extensionString(arguments, 0, "path")
		if path == "" {
			return nil, errors.New("getDefaultApplication needs a path")
		}
		name, executable, ok := windows.DefaultApplicationForPath(path)
		if !ok {
			return nil, fmt.Errorf("No application is registered to open %s", path)
		}
		return map[string]any{
			"name": name, "path": executable, "bundleId": nil, "localizedName": name,
		}, nil
	}
	return nil, fmt.Errorf("Unknown applications call %s.", method)
}

func (c *controller) extensionLaunchCommand(arguments []extension.Value) error {
	if c.extensions == nil {
		return errors.New("Extensions are unavailable")
	}
	object := extensionObject(arguments, 0)
	if object == nil {
		return errors.New("launchCommand needs a command")
	}
	name, _ := object["name"].StringValue()
	extensionName, _ := object["extensionName"].StringValue()
	if name == "" {
		return errors.New("launchCommand needs a command name")
	}
	if extensionName == "" {
		for _, entry := range c.extensions.Commands() {
			if entry.Command.Name == name || entry.Command.Title == name {
				extensionName = entry.Extension.ID()
				name = entry.Command.Name
				break
			}
		}
	}
	if extensionName == "" {
		return fmt.Errorf("Command %s is not installed.", name)
	}
	entryID := extensions.CommandRef{ExtensionName: extensionName, CommandName: name}.EntryID()
	c.startExtension(entryID)
	return nil
}

// extensionWindow answers WindowManagement's capability calls. Windows has
// one virtual desktop as far as the public API goes, so every window reports
// the same desktop id.
func (c *controller) extensionWindow(method string, arguments []extension.Value) (any, error) {
	const desktopID = "windows-desktop-1"
	describe := func(window windows.ExtensionWindow) map[string]any {
		payload := map[string]any{
			"id": window.ID,
			"bounds": map[string]any{
				"position": map[string]any{"x": float64(window.X), "y": float64(window.Y)},
				"size":     map[string]any{"width": float64(window.Width), "height": float64(window.Height)},
			},
			"desktopId":          desktopID,
			"fullScreenSettable": window.FullScreenSettable,
			"resizable":          window.Resizable,
			"positionable":       window.Positionable,
			"active":             window.Active,
		}
		if path := windows.ProcessImagePath(window.Process); path != "" {
			name := strings.TrimSuffix(filepath.Base(path), filepath.Ext(path))
			payload["application"] = map[string]any{
				"name": name, "path": path, "bundleId": nil, "localizedName": name,
			}
		}
		return payload
	}
	switch method {
	case "desktops":
		width, height := windows.ExtensionPrimaryScreen()
		return []any{map[string]any{
			"size":     map[string]any{"width": float64(width), "height": float64(height)},
			"id":       desktopID,
			"screenId": "primary",
			"active":   true,
			"type":     "User",
		}}, nil
	case "active":
		if c.target.Window != 0 {
			if window, ok := windows.ExtensionWindowByHandle(c.target.Window); ok {
				return describe(window), nil
			}
		}
		window, ok := windows.ExtensionActiveWindow()
		if !ok {
			return nil, errors.New("No window is active")
		}
		return describe(window), nil
	case "list":
		windowsOnDesktop := windows.ExtensionWindows()
		result := make([]any, 0, len(windowsOnDesktop))
		for _, window := range windowsOnDesktop {
			result = append(result, describe(window))
		}
		return result, nil
	case "setBounds":
		object := extensionObject(arguments, 0)
		if object == nil {
			return nil, errors.New("setWindowBounds needs a window id")
		}
		id, _ := object["id"].StringValue()
		if id == "" {
			return nil, errors.New("setWindowBounds needs a window id")
		}
		raw, ok := object["bounds"]
		if !ok {
			return nil, errors.New("setWindowBounds needs bounds")
		}
		if text, _ := raw.StringValue(); text == "fullscreen" {
			if !windows.ExtensionMaximizeWindow(id) {
				return nil, errors.New("The window could not be made fullscreen")
			}
			return nil, nil
		}
		bounds, ok := raw.ObjectValue()
		if !ok {
			return nil, errors.New("setWindowBounds bounds must be an object or fullscreen")
		}
		var x, y, width, height float64
		var hasX, hasY, hasWidth, hasHeight bool
		if position, ok := bounds["position"].ObjectValue(); ok {
			if value, ok := position["x"]; ok {
				x, hasX = value.DoubleValue()
			}
			if value, ok := position["y"]; ok {
				y, hasY = value.DoubleValue()
			}
		}
		if size, ok := bounds["size"].ObjectValue(); ok {
			if value, ok := size["width"]; ok {
				width, hasWidth = value.DoubleValue()
			}
			if value, ok := size["height"]; ok {
				height, hasHeight = value.DoubleValue()
			}
		}
		if !windows.ExtensionSetWindowBounds(id, x, y, width, height, hasX, hasY, hasWidth, hasHeight) {
			return nil, errors.New("The window could not be moved")
		}
		return nil, nil
	}
	return nil, fmt.Errorf("Unknown window call %s.", method)
}

// extensionRemoteImage returns a cached bitmap for an http(s) image and starts
// one background download when it is missing. Returning nil on the first
// frame keeps the UI thread free; the panel repaints when the decode lands.
func (c *controller) extensionRemoteImage(rawURL string) *ui.Bitmap {
	if !strings.HasPrefix(rawURL, "http://") && !strings.HasPrefix(rawURL, "https://") {
		return nil
	}
	c.extensionImagesMu.Lock()
	if bitmap, ok := c.extensionImages[rawURL]; ok {
		c.extensionImagesMu.Unlock()
		return bitmap
	}
	if c.extensionImagesTried[rawURL] {
		c.extensionImagesMu.Unlock()
		return nil
	}
	if c.extensionImages == nil {
		c.extensionImages = make(map[string]*ui.Bitmap)
		c.extensionImagesTried = make(map[string]bool)
	}
	c.extensionImagesTried[rawURL] = true
	c.extensionImagesMu.Unlock()

	go func() {
		client := &http.Client{Timeout: 15 * time.Second}
		request, err := http.NewRequest(http.MethodGet, rawURL, nil)
		if err != nil {
			return
		}
		request.Header.Set("User-Agent", "Wincast/"+appVersion())
		response, err := client.Do(request)
		if err != nil {
			return
		}
		defer response.Body.Close()
		if response.StatusCode < 200 || response.StatusCode >= 300 {
			return
		}
		data, err := io.ReadAll(io.LimitReader(response.Body, 12<<20))
		if err != nil {
			return
		}
		bitmap, err := ui.DecodeBitmap(data)
		if err != nil || bitmap == nil {
			return
		}
		c.extensionImagesMu.Lock()
		c.extensionImages[rawURL] = bitmap
		c.extensionImagesMu.Unlock()
		if c.panel != nil {
			c.panel.Update(func() {})
		}
	}()
	return nil
}

func extensionObject(arguments []extension.Value, index int) map[string]extension.Value {
	if index >= len(arguments) {
		return nil
	}
	object, _ := arguments[index].ObjectValue()
	return object
}

func extensionString(arguments []extension.Value, index int, key string) string {
	object := extensionObject(arguments, index)
	if object == nil {
		return ""
	}
	value, _ := object[key].StringValue()
	return value
}

func extensionStringArray(arguments []extension.Value, index int, key string) []string {
	object := extensionObject(arguments, index)
	if object == nil {
		return nil
	}
	values, _ := object[key].ArrayValue()
	result := make([]string, 0, len(values))
	for _, value := range values {
		text, _ := value.StringValue()
		if text != "" {
			result = append(result, text)
		}
	}
	return result
}
