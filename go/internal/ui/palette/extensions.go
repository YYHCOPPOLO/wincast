package palette

import (
	"encoding/base64"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/components"
	"github.com/YYHCOPPOLO/wincast/go/internal/ui/theme"
	"github.com/egoist/mygo/ui"
	"github.com/yuin/goldmark"
	"github.com/yuin/goldmark/ast"
	"github.com/yuin/goldmark/text"
)

type extensionScreenKind uint8

const (
	extensionList extensionScreenKind = iota
	extensionGrid
	extensionDetail
	extensionForm
	extensionUnsupported
)

type extensionItem struct {
	Node  *extension.Node
	Index int
}

type extensionRow struct {
	Header   bool
	Title    string
	Subtitle string
	ID       string
	Item     *extensionItem
}

type extensionSection struct {
	Title    string
	Subtitle string
	Items    []*extensionItem
}

// ExtensionScreen is the single source of truth for extension row order, so
// the flat selection index maps one-to-one onto the visible items.
type ExtensionScreen struct {
	Kind             extensionScreenKind
	Root             *extension.Node
	Rows             []extensionRow
	Sections         []extensionSection
	Items            []*extensionItem
	Fields           []*extension.Node
	GridColumns      int
	GridAspect       float64
	GridFills        bool
	Loading          bool
	NavigationTitle  string
	Placeholder      string
	FiltersLocally   bool
	SearchHandler    string
	SelectionHandler string
	ShowsDetail      bool
	ScreenActions    *extension.Node
	SearchAccessory  *extension.Node
	EmptyView        *extension.Node
	Unsupported      string
}

type ExtensionAction struct {
	Node        *extension.Node
	Title       string
	Section     string
	Handler     string
	Destructive bool
	Shortcut    string
}

type extensionFieldState struct {
	Value      string
	Bool       bool
	Values     []string
	Date       time.Time
	DateBuffer *ui.TextBuffer
	Focused    bool
	Buffer     *ui.TextBuffer
}

func (v *View) EnsureExtension() {
	if v.extensionForm == nil {
		v.extensionForm = make(map[string]*extensionFieldState)
	}
	if v.extensionIconCache == nil {
		v.extensionIconCache = make(map[string]*ui.Bitmap)
	}
	if v.extensionVectorCache == nil {
		v.extensionVectorCache = make(map[string]*ui.SVG)
	}
	if v.extensionSplit == 0 {
		v.extensionSplit = 55
	}
}

// SetExtensionTree is called on the UI thread with each committed scene.
func (v *View) SetExtensionTree(tree *extension.Tree) {
	v.EnsureExtension()
	v.ExtensionTree = tree
	v.ExtensionLoading = false
	v.rebuildExtension()
	if tree.Navigation != nil {
		if value, ok := tree.Navigation.Props["closeMainWindow"]; ok && value.Kind != extension.ValueNull {
			if data, err := json.Marshal(value); err == nil && string(data) != v.extensionCloseSeen {
				v.extensionCloseSeen = string(data)
				if v.callbacks.ExtensionClose != nil {
					v.callbacks.ExtensionClose()
				} else if v.callbacks.Hide != nil {
					v.callbacks.Hide()
				}
			}
		}
		if value, ok := tree.Navigation.Props["clearSearchBar"]; ok && value.Kind != extension.ValueNull {
			if data, err := json.Marshal(value); err == nil && string(data) != v.extensionClearSeen {
				v.extensionClearSeen = string(data)
				v.SetQuery("")
			}
		}
	}
}

// SetExtensionError shows the runtime's failure instead of a blank palette.
func (v *View) SetExtensionError(message string) {
	v.EnsureExtension()
	v.ExtensionError = message
	v.ExtensionLoading = false
}

// SetExtensionLoading clears a previous failure and marks a launch in flight.
func (v *View) SetExtensionLoading(title string) {
	v.EnsureExtension()
	v.ExtensionSession = ""
	v.ExtensionTree = nil
	v.ExtensionScreen = nil
	v.ExtensionTitle = title
	v.ExtensionLoading = true
	v.ExtensionError = ""
	v.ExtensionNotice = ""
	v.ExtensionChosen = 0
	v.ExtensionActions = false
	v.ExtensionActionAt = 0
	v.Query = ""
	if v.inputText != nil {
		v.inputText.Set("")
	}
	v.Mode = ModeExtension
}

// LeaveExtension clears every per-command extension value.
func (v *View) LeaveExtension() {
	v.ExtensionSession = ""
	v.ExtensionTree = nil
	v.ExtensionScreen = nil
	v.ExtensionTitle = ""
	v.ExtensionLoading = false
	v.ExtensionError = ""
	v.ExtensionNotice = ""
	v.ExtensionChosen = 0
	v.ExtensionActions = false
	v.ExtensionActionAt = 0
	v.Query = ""
	if v.inputText != nil {
		v.inputText.Set("")
	}
	v.Mode = ModeLauncher
}

func (v *View) rebuildExtension() {
	if v.ExtensionTree == nil {
		v.ExtensionScreen = nil
		return
	}
	previous := v.ExtensionScreen
	v.ExtensionScreen = BuildExtensionScreen(v.ExtensionTree, v.Query)
	if v.ExtensionScreen == nil {
		return
	}
	if previous != nil && previous.SearchHandler != "" && v.ExtensionScreen.SearchHandler == "" {
		// The command took the search text over; a locally filtered screen
		// starts from the same query the user already typed.
	}
	if len(v.ExtensionScreen.Items) == 0 {
		v.ExtensionChosen = -1
	} else if v.ExtensionChosen >= len(v.ExtensionScreen.Items) {
		v.ExtensionChosen = len(v.ExtensionScreen.Items) - 1
	}
	if v.ExtensionChosen < 0 && len(v.ExtensionScreen.Items) > 0 {
		v.ExtensionChosen = 0
	}
}

// BuildExtensionScreen flattens the active scene the way Raycast's palette
// does: one list of selectable items, section headers in draw order, and
// local fuzzy filtering unless the command owns onSearchTextChange.
func BuildExtensionScreen(tree *extension.Tree, query string) *ExtensionScreen {
	root := tree.ActiveRoot()
	if root == nil {
		return nil
	}
	screen := &ExtensionScreen{
		Root:             root,
		Loading:          root.BoolProp("isLoading"),
		NavigationTitle:  root.TextProp("navigationTitle"),
		Placeholder:      root.TextProp("searchBarPlaceholder"),
		SearchHandler:    root.Handler("onSearchTextChange"),
		SelectionHandler: root.Handler("onSelectionChange"),
		ShowsDetail:      root.BoolProp("isShowingDetail"),
		ScreenActions:    root.NodeProp("actions"),
		SearchAccessory:  root.NodeProp("searchBarAccessory"),
	}
	screen.FiltersLocally = screen.SearchHandler == ""
	if filtering, ok := root.Bool("filtering"); ok {
		screen.FiltersLocally = filtering && screen.SearchHandler == ""
	}
	switch root.Type {
	case "List":
		screen.Kind = extensionList
		buildExtensionItems(screen, root, "List.Item", "List.Section", "List.EmptyView", query)
	case "Grid":
		screen.Kind = extensionGrid
		screen.GridColumns = extensionGridColumns(root)
		screen.GridAspect = extensionGridAspect(root)
		screen.GridFills = root.TextProp("fit") == "fill"
		buildExtensionItems(screen, root, "Grid.Item", "Grid.Section", "Grid.EmptyView", query)
	case "Detail":
		screen.Kind = extensionDetail
	case "Form":
		screen.Kind = extensionForm
		for _, child := range root.Children {
			if strings.HasPrefix(child.Type, "Form.") {
				screen.Fields = append(screen.Fields, child)
			}
		}
	default:
		screen.Kind = extensionUnsupported
		screen.Unsupported = fmt.Sprintf("%s commands aren't supported in Wincast yet.", root.Type)
	}
	return screen
}

func buildExtensionItems(screen *ExtensionScreen, root *extension.Node, itemType, sectionType, emptyType, query string) {
	screen.EmptyView = root.NodeProp(emptyType)
	needle := strings.ToLower(strings.TrimSpace(query))
	matches := func(item *extension.Node) bool {
		if needle == "" || !screen.FiltersLocally {
			return true
		}
		haystack := extensionNodeText(item, "title") + " " + extensionNodeText(item, "subtitle")
		for _, keyword := range item.Array("keywords") {
			text, _ := keyword.StringValue()
			haystack += " " + text
		}
		return strings.Contains(strings.ToLower(haystack), needle)
	}
	add := func(node *extension.Node, section *extensionSection) {
		item := &extensionItem{Node: node, Index: len(screen.Items)}
		screen.Items = append(screen.Items, item)
		screen.Rows = append(screen.Rows, extensionRow{ID: fmt.Sprintf("item:%d", node.ID), Item: item})
		if section != nil {
			section.Items = append(section.Items, item)
		}
	}
	var loose *extensionSection
	for _, child := range root.Children {
		if child.Type == sectionType {
			matching := make([]*extension.Node, 0)
			for _, item := range child.Children {
				if item.Type == itemType && matches(item) {
					matching = append(matching, item)
				}
			}
			if len(matching) == 0 {
				continue
			}
			loose = nil
			header := extensionRow{
				Header: true, Title: extensionNodeText(child, "title"), Subtitle: extensionNodeText(child, "subtitle"),
				ID: fmt.Sprintf("header:%d", child.ID),
			}
			screen.Rows = append(screen.Rows, header)
			section := &extensionSection{Title: header.Title, Subtitle: header.Subtitle}
			for _, item := range matching {
				add(item, section)
			}
			if len(section.Items) > 0 {
				screen.Sections = append(screen.Sections, *section)
			}
			continue
		}
		if child.Type == itemType && matches(child) {
			if loose == nil {
				screen.Sections = append(screen.Sections, extensionSection{})
				loose = &screen.Sections[len(screen.Sections)-1]
			}
			add(child, loose)
		}
	}
	if len(screen.Sections) == 0 && len(screen.Items) > 0 {
		screen.Sections = append(screen.Sections, extensionSection{Items: screen.Items})
	}
}

// extensionGridColumns reads `columns` and the legacy `itemSize` prop.
func extensionGridColumns(root *extension.Node) int {
	if columns, ok := root.Double("columns"); ok && columns >= 1 {
		return min(max(int(columns), 1), 8)
	}
	switch root.TextProp("itemSize") {
	case "small":
		return 8
	case "large":
		return 3
	default:
		return 5
	}
}

// extensionGridAspect reads the tile's width-to-height ratio prop, including
// the "16/9" form extensions use.
func extensionGridAspect(root *extension.Node) float64 {
	text := strings.TrimSpace(root.TextProp("aspectRatio"))
	if text == "" {
		return 1
	}
	parts := strings.Split(text, "/")
	switch len(parts) {
	case 1:
		if value, err := strconv.ParseFloat(strings.TrimSpace(parts[0]), 64); err == nil && value > 0 {
			return value
		}
	case 2:
		width, errWidth := strconv.ParseFloat(strings.TrimSpace(parts[0]), 64)
		height, errHeight := strconv.ParseFloat(strings.TrimSpace(parts[1]), 64)
		if errWidth == nil && errHeight == nil && width > 0 && height > 0 {
			return width / height
		}
	}
	return 1
}

// extensionActions flattens ActionPanel sections and submenus. The palette
// menu is single-level, as the original's is.
func extensionActions(panel *extension.Node) []ExtensionAction {
	if panel == nil {
		return nil
	}
	actions := make([]ExtensionAction, 0)
	var walk func(node *extension.Node, section string)
	walk = func(node *extension.Node, section string) {
		for _, child := range node.Children {
			switch {
			case child.Type == "Action" || strings.HasPrefix(child.Type, "Action."):
				action := ExtensionAction{
					Node: child, Title: extensionNodeText(child, "title"), Section: section,
					Handler: child.Handler("onAction"), Destructive: child.TextProp("style") == "destructive",
				}
				if action.Title == "" {
					action.Title = "Action"
				}
				if shortcut, ok := child.Props["shortcut"]; ok {
					action.Shortcut = extensionShortcut(shortcut)
				}
				actions = append(actions, action)
			case child.Type == "ActionPanel.Section":
				title := extensionNodeText(child, "title")
				if title == "" {
					title = section
				}
				walk(child, title)
			case child.Type == "ActionPanel.Submenu":
				title := extensionNodeText(child, "title")
				if title == "" {
					title = section
				}
				walk(child, title)
			}
		}
	}
	walk(panel, "")
	return actions
}

func (v *View) extensionVisibleActions() []ExtensionAction {
	if v.ExtensionScreen == nil {
		return nil
	}
	if v.ExtensionScreen.Kind == extensionList || v.ExtensionScreen.Kind == extensionGrid {
		if v.ExtensionChosen >= 0 && v.ExtensionChosen < len(v.ExtensionScreen.Items) {
			item := v.ExtensionScreen.Items[v.ExtensionChosen].Node
			if panel := item.NodeProp("actions"); panel != nil {
				return extensionActions(panel)
			}
		}
	}
	return extensionActions(v.ExtensionScreen.ScreenActions)
}

func (v *View) extensionPrimary() (string, bool, bool) {
	actions := v.extensionVisibleActions()
	if len(actions) == 0 {
		return v.Text("运行", "Run"), false, false
	}
	return actions[0].Title, true, actions[0].Destructive
}

func (v *View) extensionStatus() string {
	if v.ExtensionError != "" {
		return v.ExtensionError
	}
	if v.ExtensionNotice != "" {
		return v.ExtensionNotice
	}
	if v.ExtensionLoading {
		return v.Text("正在启动…", "Starting…")
	}
	if v.ExtensionScreen == nil {
		return v.ExtensionTitle
	}
	if v.ExtensionScreen.NavigationTitle != "" {
		return v.ExtensionScreen.NavigationTitle
	}
	if v.ExtensionTitle != "" {
		return v.ExtensionTitle
	}
	return v.Text("扩展", "Extension")
}

func (v *View) extensionPlaceholder() string {
	if v.ExtensionScreen != nil && v.ExtensionScreen.Placeholder != "" {
		return v.ExtensionScreen.Placeholder
	}
	return v.Text("搜索…", "Search…")
}

func (v *View) extensionHints() string {
	if v.ExtensionActions {
		return v.Text("↑↓ 选择  ↵ 运行  Esc 关闭", "↑↓ choose  ↵ run  Esc close")
	}
	if len(v.extensionVisibleActions()) > 0 {
		return v.Text("↵ 运行  Ctrl+K 操作", "↵ run  Ctrl+K actions")
	}
	return v.Text("Esc 返回", "Esc back")
}

// extensionMenuRequest opens the extension's action panel instead of the
// application menu.
func (v *View) extensionMenuRequest() {
	if v.Mode != ModeExtension {
		if v.callbacks.Menu != nil {
			v.callbacks.Menu()
		}
		return
	}
	if v.ExtensionActions {
		v.ExtensionActions = false
		return
	}
	if len(v.extensionVisibleActions()) > 0 {
		v.ExtensionActions = true
		v.ExtensionActionAt = 0
	}
}

func (v *View) extensionDispatch(handler string, args ...any) {
	if v.ExtensionSession == "" || handler == "" || v.callbacks.ExtensionDispatch == nil {
		return
	}
	payload, err := json.Marshal(args)
	if err != nil {
		return
	}
	v.callbacks.ExtensionDispatch(v.ExtensionSession, handler, string(payload))
}

func (v *View) extensionRunAction(action ExtensionAction) {
	if action.Handler == "" {
		return
	}
	v.ExtensionActions = false
	v.extensionDispatch(action.Handler)
}

func (v *View) extensionOpen() {
	if v.ExtensionActions {
		actions := v.extensionVisibleActions()
		if v.ExtensionActionAt >= 0 && v.ExtensionActionAt < len(actions) {
			v.extensionRunAction(actions[v.ExtensionActionAt])
		}
		return
	}
	if v.ExtensionScreen == nil {
		return
	}
	switch v.ExtensionScreen.Kind {
	case extensionForm:
		actions := extensionActions(v.ExtensionScreen.ScreenActions)
		for _, action := range actions {
			if action.Node.Type == "Action.SubmitForm" || action.Title == "Submit" {
				v.extensionRunAction(action)
				return
			}
		}
		if len(actions) > 0 {
			v.extensionRunAction(actions[0])
		}
	case extensionUnsupported:
		return
	default:
		actions := v.extensionVisibleActions()
		if len(actions) > 0 {
			v.extensionRunAction(actions[0])
		}
	}
}

func (v *View) extensionEscape() bool {
	if v.ExtensionActions {
		v.ExtensionActions = false
		return true
	}
	if v.Mode != ModeExtension {
		return false
	}
	if v.Query != "" {
		v.SetQuery("")
		v.Input.Focus()
		return true
	}
	if v.callbacks.ExtensionPop != nil && v.callbacks.ExtensionPop(v.ExtensionSession) {
		return true
	}
	if v.callbacks.ExtensionLeave != nil {
		v.callbacks.ExtensionLeave()
	} else {
		v.LeaveExtension()
	}
	return true
}

func (v *View) extensionMove(step int) {
	if v.ExtensionActions {
		actions := v.extensionVisibleActions()
		if len(actions) == 0 {
			return
		}
		v.ExtensionActionAt = (v.ExtensionActionAt + step + len(actions)) % len(actions)
		return
	}
	if v.ExtensionScreen == nil || len(v.ExtensionScreen.Items) == 0 {
		return
	}
	if v.ExtensionScreen.Kind == extensionGrid {
		v.extensionMoveGrid(step, true)
		return
	}
	next := v.ExtensionChosen + step
	if next < 0 {
		next = 0
	}
	if next >= len(v.ExtensionScreen.Items) {
		next = len(v.ExtensionScreen.Items) - 1
	}
	v.ExtensionChosen = next
}

func (v *View) extensionMoveHorizontal(step int) {
	if v.ExtensionActions {
		return
	}
	if v.ExtensionScreen == nil || v.ExtensionScreen.Kind != extensionGrid ||
		len(v.ExtensionScreen.Items) == 0 {
		return
	}
	v.extensionMoveGrid(step, false)
}

// extensionMoveGrid moves the flat selection within a section's rows, keeping
// the column when crossing section boundaries, as Raycast's grid does.
func (v *View) extensionMoveGrid(step int, vertical bool) {
	screen := v.ExtensionScreen
	if screen == nil || len(screen.Items) == 0 {
		return
	}
	columns := max(screen.GridColumns, 1)
	if screen.Kind != extensionGrid {
		columns = 1
	}
	current := v.ExtensionChosen
	if current < 0 || current >= len(screen.Items) {
		current = 0
	}
	starts := make([]int, 0, len(screen.Sections))
	counts := make([]int, 0, len(screen.Sections))
	running := 0
	for _, section := range screen.Sections {
		if len(section.Items) == 0 {
			continue
		}
		starts = append(starts, running)
		counts = append(counts, len(section.Items))
		running += len(section.Items)
	}
	if len(counts) == 0 {
		return
	}
	section := 0
	for index := range starts {
		if current >= starts[index] {
			section = index
		}
	}
	if current >= starts[section]+counts[section] {
		return
	}
	local := current - starts[section]
	next := current
	if vertical {
		if step > 0 {
			below := local + columns
			switch {
			case below < counts[section]:
				next = starts[section] + below
			case local/columns < (counts[section]-1)/columns:
				next = starts[section] + counts[section] - 1
			case section+1 < len(counts):
				next = starts[section+1] + min(local%columns, counts[section+1]-1)
			}
		} else {
			if local >= columns {
				next = starts[section] + local - columns
			} else if section > 0 {
				previous := counts[section-1]
				lastRowStart := ((previous - 1) / columns) * columns
				next = starts[section-1] + min(lastRowStart+local%columns, previous-1)
			}
		}
	} else {
		nextLocal := local + step
		if nextLocal >= 0 && nextLocal < counts[section] {
			next = starts[section] + nextLocal
		}
	}
	if next < 0 {
		next = 0
	}
	if next >= len(screen.Items) {
		next = len(screen.Items) - 1
	}
	v.ExtensionChosen = next
}

func (v *View) extensionSearchChanged(text string) {
	if v.ExtensionScreen != nil && v.ExtensionScreen.SearchHandler != "" {
		v.extensionDispatch(v.ExtensionScreen.SearchHandler, text)
		return
	}
	if v.ExtensionTree != nil {
		v.ExtensionChosen = 0
		v.ExtensionScreen = BuildExtensionScreen(v.ExtensionTree, text)
	}
}

// extensionBody draws the active extension screen.
func (v *View) extensionBody(c *ui.Context, tokens theme.Tokens) {
	v.EnsureExtension()
	if v.ExtensionError != "" {
		ui.Column(c).Fill().Padding(24).Gap(8).Children(func() {
			ui.Text(c, v.ExtensionError).FontSize(theme.RowFont).TextColor(tokens.TextSecondary)
			ui.Text(c, v.Text("Esc 返回", "Esc back")).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
		})
		return
	}
	if v.ExtensionLoading && v.ExtensionScreen == nil {
		ui.Row(c).Fill().Center().Children(func() {
			ui.Spinner(c).Size(16, 16)
			ui.Text(c, v.Text("正在启动…", "Starting…")).FontSize(theme.RowFont).TextColor(tokens.TextSecondary)
		})
		return
	}
	if v.ExtensionScreen == nil {
		return
	}
	switch v.ExtensionScreen.Kind {
	case extensionList:
		v.extensionListBody(c, tokens)
	case extensionGrid:
		v.extensionGridBody(c, tokens)
	case extensionDetail:
		v.extensionDetailBody(c, tokens)
	case extensionForm:
		v.extensionFormBody(c, tokens)
	default:
		ui.Text(c, v.ExtensionScreen.Unsupported).FontSize(theme.RowFont).TextColor(tokens.TextSecondary).Padding(24)
	}
}

func (v *View) extensionGridBody(c *ui.Context, tokens theme.Tokens) {
	screen := v.ExtensionScreen
	if len(screen.Items) == 0 {
		if screen.EmptyView != nil {
			ui.Text(c, screen.EmptyView.TextProp("title")).FontSize(theme.RowFont).TextColor(tokens.TextSecondary).Padding(24)
			if description := screen.EmptyView.TextProp("description"); description != "" {
				ui.Text(c, description).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).Padding(24, 0)
			}
		} else if !screen.Loading {
			ui.Text(c, v.Text("没有结果", "No results")).FontSize(theme.RowFont).TextColor(tokens.TextTertiary).Padding(24)
		}
		return
	}
	ui.Scroll(c).Fill().TrackScroll(&v.extensionDetail).Children(func() {
		ui.Column(c).FillWidth().Padding(10, 8, 12).Gap(10).Children(func() {
			for _, section := range screen.Sections {
				if section.Title != "" || section.Subtitle != "" {
					title := section.Title
					if section.Subtitle != "" {
						if title != "" {
							title += "  ·  "
						}
						title += section.Subtitle
					}
					ui.Text(c, title).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Padding(4, 4, 2)
				}
				if len(section.Items) == 0 {
					continue
				}
				ui.Grid(c).FillWidth().Columns(max(screen.GridColumns, 1)).Gap(10).Children(func() {
					for _, item := range section.Items {
						v.extensionGridCell(c, tokens, item)
					}
				})
			}
		})
	})
}

func (v *View) extensionGridCell(c *ui.Context, tokens theme.Tokens, item *extensionItem) ui.Element {
	screen := v.ExtensionScreen
	node := item.Node
	selected := item.Index == v.ExtensionChosen
	element := ui.Column(c).Gap(4)
	if selected {
		element = element.Background(tokens.Selection)
	}
	element = element.Padding(4).Radius(8)
	element.Children(func() {
		tile := ui.Column(c).FillWidth().AspectRatio(float32(screen.GridAspect)).
			AlignItems(ui.Center).Justify(ui.Center).Radius(10)
		fill := tokens.CardFill
		switch {
		case selected:
			fill = tokens.Selection
		case tile.Hovered():
			fill = tokens.RowHover
		}
		tile.Background(fill)
		if selected {
			tile.Border(2, tokens.Accent)
		}
		tile.Children(func() {
			if picture, ok := v.extensionBitmap(node.Props["content"]); ok {
				fit := ui.Contain
				if screen.GridFills {
					fit = ui.Cover
				}
				ui.Image(c, picture).Fill().Fit(fit)
				return
			}
			if vector, ok := v.extensionVector(node.Props["content"]); ok {
				fit := ui.Contain
				if screen.GridFills {
					fit = ui.Cover
				}
				ui.Image(c, vector).Fill().Fit(fit)
				return
			}
			if content, ok := node.Props["content"].ObjectValue(); ok {
				if value, ok := content["value"].ObjectValue(); ok {
					content = value
				}
				if color, ok := content["color"]; ok {
					red, green, blue := extensionColorRGB(color, 127, 127, 127)
					tile.Background(ui.RGBA(red, green, blue, 255))
					return
				}
			}
			if icon, ok := node.Props["icon"]; ok {
				v.extensionIconElement(c, tokens, icon, false)
				return
			}
			v.extensionIconElement(c, tokens, extension.NullValue, false)
		})
		if title := extensionNodeText(node, "title"); title != "" {
			ui.Text(c, title).FontSize(theme.TrailingFont).TextColor(tokens.Text).SingleLine().PaddingX(2)
		}
		if subtitle := extensionNodeText(node, "subtitle"); subtitle != "" {
			ui.Text(c, subtitle).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine().PaddingX(2)
		}
	})
	if element.Clicked() {
		v.ExtensionChosen = item.Index
	}
	if selected {
		element.ScrollIntoView()
	}
	return element
}

func (v *View) extensionListBody(c *ui.Context, tokens theme.Tokens) {
	screen := v.ExtensionScreen
	if len(screen.Items) == 0 {
		if screen.EmptyView != nil {
			ui.Text(c, screen.EmptyView.TextProp("title")).FontSize(theme.RowFont).TextColor(tokens.TextSecondary).Padding(24)
			if description := screen.EmptyView.TextProp("description"); description != "" {
				ui.Text(c, description).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).Padding(24, 0)
			}
		} else if !screen.Loading {
			ui.Text(c, v.Text("没有结果", "No results")).FontSize(theme.RowFont).TextColor(tokens.TextTertiary).Padding(24)
		}
		return
	}
	if screen.ShowsDetail && v.ExtensionChosen >= 0 && v.ExtensionChosen < len(screen.Items) {
		ui.SplitPercent(c, &v.extensionSplit, func() {
			v.extensionRows(c, tokens)
		}, func() {
			v.extensionItemDetail(c, tokens, screen.Items[v.ExtensionChosen].Node.NodeProp("detail"))
		}).Fill()
		return
	}
	v.extensionRows(c, tokens)
}

func (v *View) extensionRows(c *ui.Context, tokens theme.Tokens) {
	screen := v.ExtensionScreen
	ui.Scroll(c).Fill().TrackScroll(&v.extensionDetail).Children(func() {
		ui.Column(c).FillWidth().Padding(0, 4, 8).Children(func() {
			for _, row := range screen.Rows {
				if row.Header {
					ui.Text(c, row.Title).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Padding(10, 12, 2)
					continue
				}
				if row.Item == nil {
					continue
				}
				el := v.extensionRow(c, tokens, row.Item)
				if row.Item.Index == v.ExtensionChosen {
					el.ScrollIntoView()
				}
			}
		})
	})
}

func (v *View) extensionRow(c *ui.Context, tokens theme.Tokens, item *extensionItem) ui.Element {
	node := item.Node
	selected := item.Index == v.ExtensionChosen
	title := extensionNodeText(node, "title")
	subtitle := extensionNodeText(node, "subtitle")
	element := ui.Row(c).Height(theme.RowHeight).Padding(6, 8).Gap(10).AlignItems(ui.Center).Radius(theme.RowRadius)
	fill := ui.Transparent
	switch {
	case selected:
		fill = tokens.Selection
	case element.Hovered():
		fill = tokens.RowHover
	}
	element.Background(fill).Children(func() {
		v.extensionIconElement(c, tokens, node.Props["icon"], false)
		ui.Column(c).Grow(1).Gap(1).Children(func() {
			ui.Text(c, title).FontSize(theme.RowFont).TextColor(tokens.Text).SingleLine()
			if subtitle != "" {
				ui.Text(c, subtitle).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary).SingleLine()
			}
		})
		if accessories := node.Array("accessories"); len(accessories) > 0 {
			ui.Row(c).Gap(6).Children(func() {
				for _, accessory := range accessories {
					if text := extensionAccessoryText(accessory); text != "" {
						ui.Text(c, text).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).SingleLine()
					}
				}
			})
		}
	})
	if element.Clicked() {
		v.ExtensionChosen = item.Index
	}
	return element
}

func (v *View) extensionItemDetail(c *ui.Context, tokens theme.Tokens, detail *extension.Node) {
	if detail == nil {
		return
	}
	ui.Scroll(c).Fill().TrackScroll(&v.extensionDetail).Children(func() {
		ui.Column(c).FillWidth().Padding(18, 18).Children(func() {
			if markdown := detail.TextProp("markdown"); markdown != "" {
				v.extensionMarkdown(c, tokens, markdown)
			}
			if metadata := detail.NodeProp("metadata"); metadata != nil {
				v.extensionMetadata(c, tokens, metadata)
			}
		})
	})
}

func (v *View) extensionDetailBody(c *ui.Context, tokens theme.Tokens) {
	root := v.ExtensionScreen.Root
	ui.Scroll(c).Fill().TrackScroll(&v.extensionDetail).Children(func() {
		ui.Column(c).FillWidth().Padding(18, 18).Children(func() {
			if root.TextProp("navigationTitle") != "" {
				ui.Text(c, root.TextProp("navigationTitle")).FontSize(theme.SectionFont).FontWeight(600).TextColor(tokens.Text)
			}
			v.extensionMarkdown(c, tokens, root.TextProp("markdown"))
			if metadata := root.NodeProp("metadata"); metadata != nil {
				v.extensionMetadata(c, tokens, metadata)
			}
		})
	})
}

func (v *View) extensionFormBody(c *ui.Context, tokens theme.Tokens) {
	ui.Scroll(c).Fill().TrackScroll(&v.extensionDetail).Children(func() {
		ui.Column(c).FillWidth().Padding(18, 18).Gap(10).Children(func() {
			ui.Form(c, func() {
				for _, field := range v.ExtensionScreen.Fields {
					v.extensionFormField(c, tokens, field)
				}
			})
		})
	})
}

func (v *View) extensionFormField(c *ui.Context, tokens theme.Tokens, node *extension.Node) {
	label := extensionNodeText(node, "title")
	if label == "" {
		label = node.TextProp("id")
	}
	state := v.extensionForm[node.TextProp("id")]
	if state == nil {
		state = &extensionFieldState{Buffer: ui.NewTextBuffer(""), DateBuffer: ui.NewTextBuffer("")}
		rawValue, hasValue := node.Props["value"]
		if !hasValue {
			rawValue, hasValue = node.Props["defaultValue"]
		}
		if hasValue {
			state.Value, _ = rawValue.StringValue()
		}
		state.Buffer.Set(state.Value)
		switch node.Type {
		case "Form.TagPicker", "Form.FilePicker":
			if values, ok := rawValue.ArrayValue(); ok {
				for _, item := range values {
					if text, ok := item.StringValue(); ok {
						state.Values = append(state.Values, text)
					}
				}
			}
		case "Form.DatePicker":
			if date, ok := extensionDateValue(rawValue); ok {
				state.Date = date
			} else {
				state.Date = time.Now()
			}
			if node.TextProp("type") == "date" {
				state.Date = time.Date(state.Date.Year(), state.Date.Month(), state.Date.Day(), 0, 0, 0, 0, state.Date.Location())
			}
			state.DateBuffer.Set(state.Date.Format("15:04"))
		}
		v.extensionForm[node.TextProp("id")] = state
	}
	switch node.Type {
	case "Form.Description":
		ui.Text(c, node.TextProp("text")).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
		return
	case "Form.Separator":
		components.Hairline(c, tokens)
		return
	case "Form.TextArea":
		ui.Field(c, label, func() {
			value := state.Value
			area := ui.TextArea(c, &value)
			if area.Changed() {
				state.Value = value
				if handler := node.Handler("onChange"); handler != "" {
					v.extensionDispatch(handler, state.Value)
				}
			}
		})
		return
	case "Form.Checkbox":
		checked := state.Bool
		ui.Field(c, label, func() {
			box := ui.CheckboxBase(c, &checked).Changed()
			if box {
				state.Bool = checked
				if handler := node.Handler("onChange"); handler != "" {
					v.extensionDispatch(handler, checked)
				}
			}
		})
		return
	case "Form.Dropdown":
		values := make([]string, 0)
		titles := make([]string, 0)
		for _, child := range node.Children {
			if child.Type != "Form.Dropdown.Item" {
				continue
			}
			value := child.TextProp("value")
			if value == "" {
				value = extensionNodeText(child, "title")
			}
			values = append(values, value)
			titles = append(titles, extensionNodeText(child, "title"))
		}
		selected := state.Value
		ui.Field(c, label, func() {
			choice := ui.Select(c, &selected, titles).Changed()
			if choice {
				for index, title := range titles {
					if title == selected {
						state.Value = values[index]
						break
					}
				}
				if handler := node.Handler("onChange"); handler != "" {
					v.extensionDispatch(handler, state.Value)
				}
			}
		})
		return
	case "Form.TagPicker":
		items := extensionPickerItems(node)
		chosen := make(map[string]bool, len(state.Values))
		for _, value := range state.Values {
			chosen[value] = true
		}
		labelText := node.TextProp("placeholder")
		if labelText == "" {
			labelText = v.Text("选择…", "Select…")
		}
		titles := make([]string, 0, len(state.Values))
		for _, item := range items {
			if chosen[item.Value] {
				titles = append(titles, item.Title)
			}
		}
		if len(titles) > 0 {
			labelText = strings.Join(titles, ", ")
		}
		ui.Field(c, label, func() {
			ui.MenuButton(c, labelText, func(menu *ui.Menu) {
				lastSection := ""
				for _, item := range items {
					if item.Section != "" && item.Section != lastSection {
						if lastSection != "" {
							menu.Separator()
						}
						lastSection = item.Section
					}
					entry := menu.Item(item.Title).Checked(chosen[item.Value])
					if entry.Chosen() {
						if chosen[item.Value] {
							delete(chosen, item.Value)
						} else {
							chosen[item.Value] = true
						}
						state.Values = state.Values[:0]
						for _, candidate := range items {
							if chosen[candidate.Value] {
								state.Values = append(state.Values, candidate.Value)
							}
						}
						if handler := node.Handler("onChange"); handler != "" {
							v.extensionDispatch(handler, state.Values)
						}
						break
					}
				}
			}).AlignSelf(ui.Start)
		})
		return
	case "Form.FilePicker":
		display := v.Text("选择文件…", "Choose…")
		if len(state.Values) > 0 {
			names := make([]string, 0, len(state.Values))
			for _, path := range state.Values {
				names = append(names, filepath.Base(path))
			}
			display = strings.Join(names, ", ")
		}
		ui.Field(c, label, func() {
			if ui.Button(c, display).AlignSelf(ui.Start).Clicked() {
				if v.callbacks.ExtensionPickFiles != nil {
					directories := node.BoolProp("canChooseDirectories")
					files := true
					if canChoose, ok := node.Bool("canChooseFiles"); ok {
						files = canChoose
					}
					multiple := true
					if allowMultiple, ok := node.Bool("allowMultipleSelection"); ok {
						multiple = allowMultiple
					}
					state.Values = v.callbacks.ExtensionPickFiles(multiple, directories, files)
					if handler := node.Handler("onChange"); handler != "" {
						v.extensionDispatch(handler, state.Values)
					}
				}
			}
		})
		return
	case "Form.DatePicker":
		kind := node.TextProp("type")
		ui.Field(c, label, func() {
			ui.Row(c).Gap(8).AlignItems(ui.Center).Children(func() {
				if kind != "time" {
					if ui.DateInput(c, &state.Date).Changed() {
						if kind == "date" {
							state.Date = time.Date(state.Date.Year(), state.Date.Month(), state.Date.Day(), 0, 0, 0, 0, state.Date.Location())
						}
						state.DateBuffer.Set(state.Date.Format("15:04"))
						if handler := node.Handler("onChange"); handler != "" {
							v.extensionDispatch(handler, state.Date.Format(time.RFC3339))
						}
					}
				}
				if kind != "date" {
					input := ui.TextInputBufferBase(c, state.DateBuffer).Width(90).Placeholder("HH:MM")
					if input.Changed() {
						if parsed, err := time.Parse("15:04", strings.TrimSpace(state.DateBuffer.String())); err == nil {
							state.Date = time.Date(state.Date.Year(), state.Date.Month(), state.Date.Day(),
								parsed.Hour(), parsed.Minute(), 0, 0, state.Date.Location())
							if handler := node.Handler("onChange"); handler != "" {
								v.extensionDispatch(handler, state.Date.Format(time.RFC3339))
							}
						}
					}
				}
			})
		})
		return
	case "Form.LinkAccessory":
		if text := extensionNodeText(node, "text"); text != "" {
			ui.Text(c, text).FontSize(theme.TrailingFont).TextColor(tokens.Accent)
		}
		return
	}
	password := node.Type == "Form.PasswordField"
	ui.Field(c, label, func() {
		input := ui.TextInputBufferBase(c, state.Buffer).Grow(1)
		if password {
			input = input.Password()
		}
		if input.Changed() {
			state.Value = state.Buffer.String()
			if handler := node.Handler("onChange"); handler != "" {
				v.extensionDispatch(handler, state.Value)
			}
		}
	})
}

func (v *View) extensionIconElement(c *ui.Context, tokens theme.Tokens, value extension.Value, small bool) {
	size := float32(theme.ResultRowIcon)
	if small {
		size = 14
	}
	if icon, ok := v.extensionBitmap(value); ok {
		ui.Image(c, icon).Size(size, size).Fit(ui.Contain).Role(ui.RoleNone)
		return
	}
	if vector, ok := v.extensionVector(value); ok {
		ui.Image(c, vector).Size(size, size).Fit(ui.Contain).Role(ui.RoleNone)
		return
	}
	glyph := extensionGlyph(value)
	ui.Text(c, glyph).Font("Segoe Fluent Icons").FontSize(size * 0.8).TextColor(tokens.TextSecondary).Width(size).TextAlign(ui.Center)
}

// extensionVector reads an SVG artwork file; Raycast extensions ship most of
// their icons in that format.
func (v *View) extensionVector(value extension.Value) (*ui.SVG, bool) {
	path, ok := extensionImagePath(value)
	if !ok {
		return nil, false
	}
	v.EnsureExtension()
	if !filepath.IsAbs(path) {
		path = filepath.Join(v.ExtensionAssets, path)
	}
	if !strings.HasSuffix(strings.ToLower(path), ".svg") {
		return nil, false
	}
	if cached := v.extensionVectorCache[path]; cached != nil {
		return cached, true
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, false
	}
	vector, err := ui.ParseSVG(data)
	if err != nil {
		return nil, false
	}
	v.extensionVectorCache[path] = vector
	return vector, true
}

func (v *View) extensionBitmap(value extension.Value) (*ui.Bitmap, bool) {
	path, ok := extensionImagePath(value)
	if !ok {
		return nil, false
	}
	v.EnsureExtension()
	if strings.HasPrefix(path, "data:") {
		if cached := v.extensionIconCache[path]; cached != nil {
			return cached, true
		}
		data, err := decodeDataURL(path)
		if err != nil {
			return nil, false
		}
		if bitmap, err := decodeBitmap(data); err == nil {
			v.extensionIconCache[path] = bitmap
			return bitmap, true
		}
		return nil, false
	}
	if strings.HasPrefix(path, "http://") || strings.HasPrefix(path, "https://") {
		if cached := v.extensionIconCache[path]; cached != nil {
			return cached, true
		}
		if v.callbacks.ExtensionImage == nil {
			return nil, false
		}
		if bitmap := v.callbacks.ExtensionImage(path); bitmap != nil {
			v.extensionIconCache[path] = bitmap
			return bitmap, true
		}
		return nil, false
	}
	if !filepath.IsAbs(path) {
		path = filepath.Join(v.ExtensionAssets, path)
	}
	if cached := v.extensionIconCache[path]; cached != nil {
		return cached, true
	}
	if strings.HasSuffix(strings.ToLower(path), ".svg") {
		return nil, false
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, false
	}
	bitmap, err := decodeBitmap(data)
	if err != nil {
		return nil, false
	}
	v.extensionIconCache[path] = bitmap
	return bitmap, true
}

func decodeBitmap(data []byte) (*ui.Bitmap, error) {
	return ui.DecodeBitmap(data)
}

func extensionImagePath(value extension.Value) (string, bool) {
	switch value.Kind {
	case extension.ValueString:
		return value.String, true
	case extension.ValueObject:
		for _, key := range []string{"source", "fileIcon", "light", "dark", "value"} {
			if nested, ok := value.Object[key]; ok {
				if path, ok := extensionImagePath(nested); ok {
					return path, true
				}
			}
		}
	}
	return "", false
}

func decodeDataURL(value string) ([]byte, error) {
	comma := strings.Index(value, ",")
	if comma < 0 {
		return nil, fmt.Errorf("invalid data URL")
	}
	header, payload := value[:comma], value[comma+1:]
	if strings.Contains(header, ";base64") {
		return base64.StdEncoding.DecodeString(payload)
	}
	decoded, err := url.QueryUnescape(payload)
	if err != nil {
		return nil, err
	}
	return []byte(decoded), nil
}

// extensionGlyph maps the common Raycast SF Symbol names to Segoe Fluent
// Icons. Unknown names get the extension puzzle.
func extensionGlyph(value extension.Value) string {
	name, _ := extensionImagePath(value)
	if name == "" {
		return "\uE7B8"
	}
	if utf8.RuneCountInString(name) <= 2 && !strings.Contains(name, ".") && !strings.Contains(name, "/") {
		return name
	}
	if glyph, ok := extensionGlyphNames[name]; ok {
		return glyph
	}
	return "\uE7B8"
}

var extensionGlyphNames = map[string]string{
	"clipboard":              "\uE8C8",
	"gear":                   "\uE713",
	"magnifyingglass":        "\uE721",
	"folder":                 "\uE8B7",
	"doc.text":               "\uE8A5",
	"globe":                  "\uE774",
	"star":                   "\uE735",
	"heart":                  "\uEB51",
	"link":                   "\uE71B",
	"trash":                  "\uE74D",
	"plus":                   "\uE710",
	"minus":                  "\uE738",
	"check":                  "\uE73E",
	"xmark":                  "\uE711",
	"arrow.clockwise":        "\uE72C",
	"person":                 "\uE77B",
	"bell":                   "\uEA8F",
	"calendar":               "\uE787",
	"clock":                  "\uE823",
	"puzzlepiece.extension":  "\uE7B8",
	"bolt":                   "\uE945",
	"doc":                    "\uE8A5",
	"list.bullet":            "\uE8FD",
	"photo":                  "\uEB9F",
	"terminal":               "\uE756",
	"gearshape":              "\uE713",
	"magnifyingglass.circle": "\uE721",
	"bookmark":               "\uE8A4",
	"envelope":               "\uE715",
	"message":                "\uE8BD",
	"cloud":                  "\uE753",
	"lock":                   "\uE72E",
	"key":                    "\uE8D7",
	"hammer":                 "\uE90F",
}

// ExtensionSymbolGlyph and ExtensionTintColor let the settings pane preview
// the same tile the launcher draws.
func ExtensionSymbolGlyph(name string) string { return extensionSymbolGlyph(name) }

func ExtensionTintColor(name string) ui.Color { return extensionTintColor(name) }

// extensionSymbolGlyph maps a stored SF Symbol name to the Fluent glyph the
// launcher draws. Unknown names fall back to the extension puzzle.
func extensionSymbolGlyph(name string) string {
	if name == "" {
		return ""
	}
	if glyph, ok := extensionGlyphNames[name]; ok {
		return glyph
	}
	return "\uE7B8"
}

// extensionTintColor maps a stored appearance tint to its fixed sRGB value,
// matching the original catalogue's swatches.
func extensionTintColor(name string) ui.Color {
	switch name {
	case "red":
		return ui.RGBA(255, 69, 59, 255)
	case "maroon":
		return ui.RGBA(158, 61, 61, 255)
	case "rose":
		return ui.RGBA(255, 115, 135, 255)
	case "pink":
		return ui.RGBA(255, 56, 94, 255)
	case "purple":
		return ui.RGBA(191, 89, 242, 255)
	case "indigo":
		return ui.RGBA(94, 92, 230, 255)
	case "blue":
		return ui.RGBA(10, 133, 255, 255)
	case "cyan":
		return ui.RGBA(99, 209, 255, 255)
	case "teal":
		return ui.RGBA(64, 199, 224, 255)
	case "mint":
		return ui.RGBA(102, 212, 207, 255)
	case "green":
		return ui.RGBA(51, 214, 74, 255)
	case "lime":
		return ui.RGBA(163, 219, 61, 255)
	case "yellow":
		return ui.RGBA(255, 214, 10, 255)
	case "orange":
		return ui.RGBA(255, 158, 10, 255)
	case "tan":
		return ui.RGBA(214, 179, 133, 255)
	case "brown":
		return ui.RGBA(171, 135, 97, 255)
	case "gray":
		return ui.RGBA(153, 153, 158, 255)
	case "slate":
		return ui.RGBA(112, 128, 148, 255)
	default:
		return ui.RGBA(191, 89, 242, 255)
	}
}

func extensionShortcut(value extension.Value) string {
	object, ok := value.ObjectValue()
	if !ok {
		return ""
	}
	if nested, ok := object["windows"]; ok {
		object, _ = nested.ObjectValue()
	} else if nested, ok := object["Windows"]; ok {
		object, _ = nested.ObjectValue()
	} else if nested, ok := object["macOS"]; ok {
		object, _ = nested.ObjectValue()
	}
	key, _ := object["key"].StringValue()
	modifiers := make([]string, 0)
	if values, ok := object["modifiers"].ArrayValue(); ok {
		for _, modifier := range values {
			text, _ := modifier.StringValue()
			switch text {
			case "ctrl", "cmd":
				modifiers = append(modifiers, "Ctrl")
			case "shift":
				modifiers = append(modifiers, "Shift")
			case "alt", "opt":
				modifiers = append(modifiers, "Alt")
			}
		}
	}
	parts := append(modifiers, strings.ToUpper(key))
	return strings.Join(parts, "+")
}

func (v *View) extensionMarkdown(c *ui.Context, tokens theme.Tokens, markdown string) {
	if strings.TrimSpace(markdown) == "" {
		return
	}
	parser := goldmark.DefaultParser()
	source := []byte(markdown)
	document := parser.Parse(text.NewReader(source))
	ast.Walk(document, func(node ast.Node, entering bool) (ast.WalkStatus, error) {
		if !entering {
			return ast.WalkContinue, nil
		}
		switch typed := node.(type) {
		case *ast.Heading:
			size := theme.SectionFont
			switch typed.Level {
			case 1:
				size = 20
			case 2:
				size = 17
			case 3:
				size = 15
			}
			ui.Text(c, string(typed.Text(source))).
				FontSize(size).FontWeight(600).TextColor(tokens.Text).Padding(8, 0, 2)
			return ast.WalkSkipChildren, nil
		case *ast.Paragraph:
			if image, ok := typed.FirstChild().(*ast.Image); ok && typed.ChildCount() == 1 {
				v.extensionMarkdownImage(c, tokens, string(image.Destination), string(image.Text(source)))
				return ast.WalkSkipChildren, nil
			}
			ui.RichText(c, extensionInline(source, typed)...).Padding(2, 0)
			return ast.WalkSkipChildren, nil
		case *ast.List:
			return ast.WalkContinue, nil
		case *ast.ListItem:
			prefix := "•  "
			if node.Parent() != nil {
				if list, ok := node.Parent().(*ast.List); ok && list.IsOrdered() {
					prefix = "1.  "
				}
			}
			ui.Row(c).Padding(2, 0).Gap(6).Children(func() {
				ui.Text(c, prefix).FontSize(theme.RowFont).TextColor(tokens.TextTertiary)
				ui.RichText(c, extensionInline(source, typed)...).Grow(1)
			})
			return ast.WalkSkipChildren, nil
		case *ast.FencedCodeBlock:
			var builder strings.Builder
			for line := 0; line < typed.Lines().Len(); line++ {
				segment := typed.Lines().At(line)
				builder.Write(segment.Value(source))
			}
			ui.Text(c, strings.TrimRight(builder.String(), "\n")).
				Font("Cascadia Mono").FontSize(theme.TrailingFont).TextColor(tokens.Text).
				Padding(8, 8).Background(tokens.CardFill).Radius(6)
			return ast.WalkSkipChildren, nil
		case *ast.CodeBlock:
			var builder strings.Builder
			for line := 0; line < typed.Lines().Len(); line++ {
				segment := typed.Lines().At(line)
				builder.Write(segment.Value(source))
			}
			ui.Text(c, strings.TrimRight(builder.String(), "\n")).
				Font("Cascadia Mono").FontSize(theme.TrailingFont).TextColor(tokens.Text).
				Padding(8, 8).Background(tokens.CardFill).Radius(6)
			return ast.WalkSkipChildren, nil
		case *ast.Blockquote:
			ui.Text(c, string(typed.Text(source))).
				FontSize(theme.RowFont).TextColor(tokens.TextSecondary).Padding(4, 10)
			return ast.WalkSkipChildren, nil
		case *ast.ThematicBreak:
			components.Hairline(c, tokens)
			return ast.WalkSkipChildren, nil
		case *ast.HTMLBlock:
			return ast.WalkSkipChildren, nil
		}
		return ast.WalkContinue, nil
	})
}

func extensionInline(source []byte, node ast.Node) []ui.Span {
	spans := make([]ui.Span, 0)
	var walk func(ast.Node, ui.Span)
	walk = func(current ast.Node, style ui.Span) {
		switch typed := current.(type) {
		case *ast.Text:
			style.Text = string(typed.Segment.Value(source))
			spans = append(spans, style)
			if typed.HardLineBreak() || typed.SoftLineBreak() {
				spans = append(spans, ui.Span{Text: "\n"})
			}
		case *ast.String:
			style.Text = string(typed.Value)
			spans = append(spans, style)
		case *ast.CodeSpan:
			style.Font = "Cascadia Mono"
			style.Background = ui.RGBA(127, 127, 127, 28)
			style.Radius = 3
			for child := current.FirstChild(); child != nil; child = child.NextSibling() {
				walk(child, style)
			}
		case *ast.Emphasis:
			if typed.Level >= 2 {
				style.Weight = 600
			} else {
				style.Italic = true
			}
			for child := current.FirstChild(); child != nil; child = child.NextSibling() {
				walk(child, style)
			}
		case *ast.Link:
			style.Underline = true
			for child := current.FirstChild(); child != nil; child = child.NextSibling() {
				walk(child, style)
			}
		case *ast.AutoLink:
			style.Underline = true
			style.Text = string(typed.Label(source))
			spans = append(spans, style)
		default:
			for child := current.FirstChild(); child != nil; child = child.NextSibling() {
				walk(child, style)
			}
		}
	}
	walk(node, ui.Span{Size: theme.RowFont})
	return spans
}

// extensionMarkdownImage draws a local, data or remote image from markdown.
// Remote bytes arrive through the host's background cache.
func (v *View) extensionMarkdownImage(c *ui.Context, tokens theme.Tokens, destination, alt string) {
	destination = strings.TrimSpace(destination)
	if destination == "" {
		return
	}
	value := extension.Value{Kind: extension.ValueString, String: destination}
	if bitmap, ok := v.extensionBitmap(value); ok {
		ui.Image(c, bitmap).MaxWidth(460).Fit(ui.Contain).Radius(6).Padding(4, 0)
		return
	}
	if vector, ok := v.extensionVector(value); ok {
		ui.Image(c, vector).MaxWidth(460).Fit(ui.Contain).Radius(6).Padding(4, 0)
		return
	}
	if alt == "" {
		alt = destination
	}
	ui.Text(c, alt).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).Padding(2, 0)
}

func (v *View) extensionMetadata(c *ui.Context, tokens theme.Tokens, metadata *extension.Node) {
	components.Hairline(c, tokens)
	ui.Column(c).FillWidth().Padding(6, 0).Gap(2).Children(func() {
		for _, child := range metadata.Children {
			switch child.Type {
			case "Detail.Metadata.Separator":
				components.Hairline(c, tokens)
			case "Detail.Metadata.Label":
				label := extensionNodeText(child, "title")
				value := extensionNodeText(child, "text")
				ui.Row(c).FillWidth().Gap(12).Children(func() {
					ui.Text(c, label).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).Width(110)
					ui.Text(c, value).FontSize(theme.TrailingFont).TextColor(tokens.Text).Grow(1)
				})
			case "Detail.Metadata.Link":
				ui.Row(c).FillWidth().Gap(12).Children(func() {
					ui.Text(c, extensionNodeText(child, "title")).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary).Width(110)
					ui.Text(c, extensionNodeText(child, "target")).FontSize(theme.TrailingFont).TextColor(tokens.Accent).Grow(1)
				})
			case "Detail.Metadata.TagList":
				ui.Row(c).FillWidth().Gap(12).Children(func() {
					ui.Text(c, extensionNodeText(child, "title")).FontSize(theme.TrailingFont).
						TextColor(tokens.TextTertiary).Width(110)
					ui.Row(c).Grow(1).Gap(4).Children(func() {
						for _, tag := range child.Children {
							text := extensionNodeText(tag, "text")
							if text == "" {
								continue
							}
							red, green, blue := extensionColorRGB(tag.Props["color"], 153, 153, 158)
							ui.Text(c, text).FontSize(theme.TrailingFont).
								TextColor(ui.RGBA(red, green, blue, 255)).
								Padding(1, 6).Radius(4).Background(ui.RGBA(red, green, blue, 38))
						}
					})
				})
			}
		}
	})
}

func extensionColorRGB(value extension.Value, fallbackRed, fallbackGreen, fallbackBlue uint8) (uint8, uint8, uint8) {
	text := ""
	switch value.Kind {
	case extension.ValueString:
		text = value.String
	case extension.ValueObject:
		for _, key := range []string{"light", "dark", "value"} {
			if nested, ok := value.Object[key]; ok {
				if candidate, ok := nested.StringValue(); ok && candidate != "" {
					text = candidate
					break
				}
			}
		}
	}
	text = strings.TrimSpace(strings.ToLower(text))
	switch text {
	case "raycast-blue":
		return 10, 132, 255
	case "raycast-green":
		return 48, 209, 88
	case "raycast-magenta":
		return 218, 58, 158
	case "raycast-orange":
		return 255, 159, 10
	case "raycast-purple":
		return 191, 90, 242
	case "raycast-red":
		return 255, 69, 58
	case "raycast-yellow":
		return 255, 214, 10
	case "raycast-primary-text":
		return 235, 235, 245
	case "raycast-secondary-text":
		return 160, 160, 170
	case "red":
		return 255, 69, 59
	case "green":
		return 51, 214, 74
	case "blue":
		return 10, 133, 255
	case "orange":
		return 255, 158, 10
	case "yellow":
		return 255, 214, 10
	case "purple":
		return 191, 89, 242
	case "pink":
		return 255, 56, 94
	case "gray", "grey":
		return 153, 153, 158
	}
	hex := strings.TrimPrefix(text, "#")
	if len(hex) == 3 || len(hex) == 6 || len(hex) == 8 {
		parse := func(part string) (uint8, bool) {
			value, err := strconv.ParseUint(part, 16, 8)
			return uint8(value), err == nil
		}
		switch len(hex) {
		case 3:
			red, okRed := parse(strings.Repeat(string(hex[0]), 2))
			green, okGreen := parse(strings.Repeat(string(hex[1]), 2))
			blue, okBlue := parse(strings.Repeat(string(hex[2]), 2))
			if okRed && okGreen && okBlue {
				return red, green, blue
			}
		case 6, 8:
			red, okRed := parse(hex[0:2])
			green, okGreen := parse(hex[2:4])
			blue, okBlue := parse(hex[4:6])
			if okRed && okGreen && okBlue {
				return red, green, blue
			}
		}
	}
	return fallbackRed, fallbackGreen, fallbackBlue
}

func (v *View) extensionActionsPanel(c *ui.Context, tokens theme.Tokens) {
	actions := v.extensionVisibleActions()
	if len(actions) == 0 {
		v.ExtensionActions = false
		return
	}
	if v.ExtensionActionAt >= len(actions) {
		v.ExtensionActionAt = len(actions) - 1
	}
	ui.Overlay(c, func() {
		panel := ui.Column(c).Absolute().Right(12).Top(theme.HeaderHeight+2*theme.HeaderPadding+8).
			Width(300).MaxHeight(360).Padding(6).Gap(2).
			Background(tokens.CardFill).Border(1, tokens.Line2).Radius(10)
		panel.Children(func() {
			ui.Scroll(c).Fill().Children(func() {
				ui.Column(c).FillWidth().Gap(2).Children(func() {
					lastSection := ""
					for index, action := range actions {
						if action.Section != "" && action.Section != lastSection {
							lastSection = action.Section
							ui.Text(c, action.Section).FontSize(theme.SectionFont).TextColor(tokens.TextTertiary).Padding(6, 8, 2)
						}
						row := ui.Row(c).Height(32).Padding(6, 8).Gap(8).AlignItems(ui.Center).Radius(6)
						fill := ui.Transparent
						switch {
						case index == v.ExtensionActionAt:
							fill = tokens.Selection
						case row.Hovered():
							fill = tokens.RowHover
						}
						row.Background(fill).Children(func() {
							ui.Text(c, action.Title).FontSize(theme.RowFont).TextColor(tokens.Text).Grow(1).SingleLine()
							if action.Shortcut != "" {
								ui.Text(c, action.Shortcut).FontSize(theme.TrailingFont).TextColor(tokens.TextTertiary)
							}
						})
						if row.Clicked() {
							v.extensionRunAction(action)
						}
					}
				})
			})
		})
	})
}

// extensionFooterPrimary keeps the palette footer in step with the action the
// extension would run.
func (v *View) extensionFooterPrimary() (string, bool, bool) {
	return v.extensionPrimary()
}

// extensionNodeText reads a string prop that may be shaped as
// {value, tooltip}, as Raycast's title and subtitle props are.
func extensionNodeText(node *extension.Node, key string) string {
	if node == nil {
		return ""
	}
	value, ok := node.Props[key]
	if !ok {
		return ""
	}
	if text, ok := value.StringValue(); ok {
		return text
	}
	if object, ok := value.ObjectValue(); ok {
		if text, ok := object["value"].StringValue(); ok {
			return text
		}
	}
	return ""
}

type extensionPickerItem struct {
	Value   string
	Title   string
	Section string
	Icon    extension.Value
}

// extensionPickerItems flattens a Dropdown/TagPicker's items and sections.
func extensionPickerItems(node *extension.Node) []extensionPickerItem {
	items := make([]extensionPickerItem, 0)
	if node == nil {
		return items
	}
	var walk func(node *extension.Node, section string)
	walk = func(current *extension.Node, section string) {
		for _, child := range current.Children {
			switch {
			case strings.HasSuffix(child.Type, ".Item"):
				value := child.TextProp("value")
				title := extensionNodeText(child, "title")
				if title == "" {
					title = value
				}
				icon, _ := child.Props["icon"]
				items = append(items, extensionPickerItem{
					Value: value, Title: title, Section: section, Icon: icon,
				})
			case strings.HasSuffix(child.Type, ".Section"):
				next := extensionNodeText(child, "title")
				if next == "" {
					next = section
				}
				walk(child, next)
			}
		}
	}
	walk(node, "")
	return items
}

// extensionDateValue reads a Form.DatePicker value, which crosses the bridge
// as a tagged ISO date or a plain ISO string.
func extensionDateValue(value extension.Value) (time.Time, bool) {
	text := ""
	switch value.Kind {
	case extension.ValueDate:
		text = value.Date
	case extension.ValueString:
		text = value.String
	case extension.ValueNumber:
		return time.Unix(int64(value.Number), 0), true
	}
	text = strings.TrimSpace(text)
	if text == "" {
		return time.Time{}, false
	}
	for _, layout := range []string{time.RFC3339Nano, time.RFC3339, "2006-01-02"} {
		if parsed, err := time.Parse(layout, text); err == nil {
			return parsed, true
		}
	}
	return time.Time{}, false
}

func (v *View) extensionSearchAccessory(c *ui.Context, tokens theme.Tokens) {
	if v.ExtensionScreen == nil || v.ExtensionScreen.SearchAccessory == nil {
		return
	}
	node := v.ExtensionScreen.SearchAccessory
	items := extensionPickerItems(node)
	if len(items) == 0 {
		return
	}
	value := node.TextProp("value")
	if value == "" {
		value = node.TextProp("defaultValue")
	}
	label := node.TextProp("placeholder")
	for _, item := range items {
		if item.Value == value {
			label = item.Title
			break
		}
	}
	if label == "" {
		label = items[0].Title
	}
	ui.MenuButton(c, label, func(menu *ui.Menu) {
		lastSection := ""
		for _, item := range items {
			if item.Section != "" && item.Section != lastSection {
				if lastSection != "" {
					menu.Separator()
				}
				lastSection = item.Section
			}
			entry := menu.Item(item.Title).Checked(item.Value == value)
			if entry.Chosen() {
				if handler := node.Handler("onChange"); handler != "" {
					v.extensionDispatch(handler, item.Value)
				}
			}
		}
	}).Height(30).MaxWidth(220)
}

func extensionAccessoryText(value extension.Value) string {
	if text, ok := value.StringValue(); ok {
		return text
	}
	object, ok := value.ObjectValue()
	if !ok {
		return ""
	}
	for _, key := range []string{"text", "tag", "date", "value"} {
		nested, ok := object[key]
		if !ok {
			continue
		}
		if text, ok := nested.StringValue(); ok {
			return text
		}
		if inner, ok := nested.ObjectValue(); ok {
			if text, ok := inner["value"].StringValue(); ok {
				return text
			}
		}
	}
	return ""
}

// extensionFeedback draws the Toast, HUD and Alert nodes Blast publishes
// beside the screen. Alerts dispatch their own Alert.Action callbacks so the
// extension's await confirmAlert(...) resumes with the answer.
func (v *View) extensionFeedback(c *ui.Context, tokens theme.Tokens) {
	if v.ExtensionTree == nil || v.ExtensionTree.Feedback == nil {
		return
	}
	for _, child := range v.ExtensionTree.Feedback.Children {
		switch child.Type {
		case "Toast":
			ui.Overlay(c, func() {
				ui.Row(c).Absolute().Bottom(70).Left(0).Right(0).Justify(ui.Center).Children(func() {
					pill := ui.Row(c).Padding(10, 14).Gap(12).AlignItems(ui.Center).
						Background(tokens.CardFill).Border(1, tokens.Line2).Radius(10)
					pill.Children(func() {
						ui.Column(c).Gap(1).Children(func() {
							ui.Text(c, extensionNodeText(child, "title")).FontSize(theme.RowFont).FontWeight(600).TextColor(tokens.Text)
							if message := extensionNodeText(child, "message"); message != "" {
								ui.Text(c, message).FontSize(theme.TrailingFont).TextColor(tokens.TextSecondary)
							}
						})
						for _, action := range child.Children {
							if action.Type != "Toast.Action" {
								continue
							}
							if ui.Button(c, extensionNodeText(action, "title")).Clicked() {
								v.extensionDispatch(action.Handler("onAction"))
							}
						}
					})
				})
			})
		case "HUD":
			ui.Overlay(c, func() {
				ui.Row(c).Absolute().Left(0).Top(0).Right(0).Bottom(0).Justify(ui.Center).AlignItems(ui.Center).Children(func() {
					ui.Text(c, extensionNodeText(child, "title")).FontSize(15).FontWeight(600).
						TextColor(tokens.Text).Padding(12, 18).Background(tokens.CardFill).
						Border(1, tokens.Line2).Radius(10)
				})
			})
		case "Alert":
			actions := make([]*extension.Node, 0, 2)
			buttons := make([]string, 0, 2)
			for _, action := range child.Children {
				if action.Type == "Alert.Action" {
					actions = append(actions, action)
					buttons = append(buttons, extensionNodeText(action, "title"))
				}
			}
			if len(buttons) == 0 {
				continue
			}
			open := true
			choice := ui.AlertDialog(c, &open, extensionNodeText(child, "title"),
				extensionNodeText(child, "message"), buttons...)
			if choice >= 0 && choice < len(actions) {
				v.extensionDispatch(actions[choice].Handler("onAction"))
			}
		}
	}
}

// extensionTime renders accessory dates relative to now, like Raycast.
func extensionTime(value extension.Value) string {
	text, ok := value.StringValue()
	if !ok {
		return ""
	}
	when, err := time.Parse(time.RFC3339, text)
	if err != nil {
		return text
	}
	return when.Format("Jan 2 15:04")
}
