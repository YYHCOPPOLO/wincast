package extension

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strconv"
	"strings"
)

// Value is one prop value from the extension runtime. Functions arrive as
// dispatch handles ("$fn"), dates as ISO strings ("$date"), and element-valued
// props (an ActionPanel on actions, a Detail.Metadata on metadata) as nodes.
type Value struct {
	Kind    ValueKind
	String  string
	Number  float64
	Bool    bool
	Date    string
	Handler string
	Array   []Value
	Object  map[string]Value
	Node    *Node
}

// NullValue is the zero Value.
var NullValue = Value{Kind: ValueNull}

// StringValue returns a scalar as text; numbers lose a needless ".0".
func (v Value) StringValue() (string, bool) {
	switch v.Kind {
	case ValueString:
		return v.String, true
	case ValueNumber:
		if v.Number == float64(int64(v.Number)) && v.Number < 1e15 && v.Number > -1e15 {
			return strconv.FormatInt(int64(v.Number), 10), true
		}
		return strconv.FormatFloat(v.Number, 'f', -1, 64), true
	case ValueBool:
		if v.Bool {
			return "true", true
		}
		return "false", true
	default:
		return "", false
	}
}

// Text is StringValue without the presence flag.
func (v Value) Text() string {
	text, _ := v.StringValue()
	return text
}

// BoolValue is the truthiness of a scalar.
func (v Value) BoolValue() (bool, bool) {
	switch v.Kind {
	case ValueBool:
		return v.Bool, true
	case ValueNumber:
		return v.Number != 0, true
	default:
		return false, false
	}
}

// DoubleValue reads a number or a numeric string.
func (v Value) DoubleValue() (float64, bool) {
	switch v.Kind {
	case ValueNumber:
		return v.Number, true
	case ValueString:
		value, err := strconv.ParseFloat(v.String, 64)
		return value, err == nil
	case ValueBool:
		if v.Bool {
			return 1, true
		}
		return 0, true
	default:
		return 0, false
	}
}

// HandlerID returns a callback handle.
func (v Value) HandlerID() string {
	if v.Kind == ValueHandler {
		return v.Handler
	}
	return ""
}

// NodesValue returns element-valued props, one or several.
func (v Value) NodesValue() []*Node {
	switch v.Kind {
	case ValueNode:
		if v.Node != nil {
			return []*Node{v.Node}
		}
	case ValueArray:
		nodes := make([]*Node, 0, len(v.Array))
		for _, item := range v.Array {
			if item.Kind == ValueNode && item.Node != nil {
				nodes = append(nodes, item.Node)
			}
		}
		return nodes
	}
	return nil
}

// ObjectValue returns an object prop.
func (v Value) ObjectValue() (map[string]Value, bool) {
	if v.Kind == ValueObject {
		return v.Object, true
	}
	return nil, false
}

// ArrayValue returns an array prop.
func (v Value) ArrayValue() ([]Value, bool) {
	if v.Kind == ValueArray {
		return v.Array, true
	}
	return nil, false
}

// MarshalJSON keeps the wire shapes the runtime understands.
func (v Value) MarshalJSON() ([]byte, error) {
	switch v.Kind {
	case ValueString:
		return json.Marshal(v.String)
	case ValueNumber:
		return json.Marshal(v.Number)
	case ValueBool:
		return json.Marshal(v.Bool)
	case ValueDate:
		return json.Marshal(map[string]string{"$date": v.Date})
	case ValueHandler:
		return json.Marshal(map[string]string{"$fn": v.Handler})
	case ValueArray:
		return json.Marshal(v.Array)
	case ValueObject:
		return json.Marshal(v.Object)
	default:
		return []byte("null"), nil
	}
}

// ParseValues decodes a JSON array of host-call arguments.
func ParseValues(data []byte) []Value {
	if len(bytes.TrimSpace(data)) == 0 {
		return nil
	}
	var decoded any
	if json.Unmarshal(data, &decoded) != nil {
		return nil
	}
	array, ok := decoded.([]any)
	if !ok {
		return []Value{valueFromJSON(decoded)}
	}
	values := make([]Value, 0, len(array))
	for _, item := range array {
		values = append(values, valueFromJSON(item))
	}
	return values
}

func valueFromJSON(json any) Value {
	switch typed := json.(type) {
	case nil:
		return NullValue
	case string:
		return Value{Kind: ValueString, String: typed}
	case float64:
		return Value{Kind: ValueNumber, Number: typed}
	case bool:
		return Value{Kind: ValueBool, Bool: typed}
	case []any:
		values := make([]Value, 0, len(typed))
		for _, item := range typed {
			values = append(values, valueFromJSON(item))
		}
		return Value{Kind: ValueArray, Array: values}
	case map[string]any:
		if handler, ok := typed["$fn"].(string); ok {
			return Value{Kind: ValueHandler, Handler: handler}
		}
		if date, ok := typed["$date"].(string); ok {
			return Value{Kind: ValueDate, Date: date}
		}
		if node, ok := nodeFromJSON(typed); ok {
			return Value{Kind: ValueNode, Node: node}
		}
		object := make(map[string]Value, len(typed))
		for key, item := range typed {
			object[key] = valueFromJSON(item)
		}
		return Value{Kind: ValueObject, Object: object}
	default:
		return NullValue
	}
}

// Node is one entry of the React tree the runtime commits.
type Node struct {
	ID       int
	Type     string
	Props    map[string]Value
	Children []*Node
	Text     string
}

// IsText reports a text node.
func (n *Node) IsText() bool { return n != nil && n.Type == "#text" }

// IsElement reports a real host element, the discriminator that keeps a plain
// prop object such as {type:"day"} from being mistaken for a node.
func (n *Node) IsElement() bool { return n != nil && !n.IsText() && n.ID > 0 }

func nodeFromJSON(object map[string]any) (*Node, bool) {
	nodeType, ok := object["type"].(string)
	if !ok || nodeType == "" {
		return nil, false
	}
	if nodeType == "#text" {
		text, _ := object["text"].(string)
		return &Node{Type: nodeType, Text: text}, true
	}
	rawID, ok := object["id"].(float64)
	if !ok || rawID <= 0 {
		return nil, false
	}
	rawProps, ok := object["props"].(map[string]any)
	if !ok {
		return nil, false
	}
	rawChildren, ok := object["children"].([]any)
	if !ok {
		return nil, false
	}
	node := &Node{ID: int(rawID), Type: nodeType, Props: make(map[string]Value, len(rawProps))}
	for key, value := range rawProps {
		node.Props[key] = valueFromJSON(value)
	}
	for _, child := range rawChildren {
		if object, ok := child.(map[string]any); ok {
			if parsed, ok := nodeFromJSON(object); ok {
				node.Children = append(node.Children, parsed)
			}
		}
	}
	return node, true
}

// String reads a string prop.
func (n *Node) String(key string) (string, bool) {
	if n == nil {
		return "", false
	}
	return n.Props[key].StringValue()
}

// TextProp is String with an empty fallback.
func (n *Node) TextProp(key string) string {
	text, _ := n.String(key)
	return text
}

// Bool reads a boolean prop.
func (n *Node) Bool(key string) (bool, bool) {
	if n == nil {
		return false, false
	}
	return n.Props[key].BoolValue()
}

// BoolProp is Bool with a false fallback.
func (n *Node) BoolProp(key string) bool {
	value, _ := n.Bool(key)
	return value
}

// Double reads a numeric prop.
func (n *Node) Double(key string) (float64, bool) {
	if n == nil {
		return 0, false
	}
	return n.Props[key].DoubleValue()
}

// Handler reads a callback handle prop.
func (n *Node) Handler(key string) string {
	if n == nil {
		return ""
	}
	return n.Props[key].HandlerID()
}

// NodeProp reads an element-valued prop.
func (n *Node) NodeProp(key string) *Node {
	if n == nil {
		return nil
	}
	nodes := n.Props[key].NodesValue()
	if len(nodes) == 0 {
		return nil
	}
	return nodes[0]
}

// NodesProp reads an element or array-of-elements prop.
func (n *Node) NodesProp(key string) []*Node {
	if n == nil {
		return nil
	}
	return n.Props[key].NodesValue()
}

// Array reads an array prop.
func (n *Node) Array(key string) []Value {
	if n == nil {
		return nil
	}
	values, _ := n.Props[key].ArrayValue()
	return values
}

// Object reads an object prop.
func (n *Node) Object(key string) (map[string]Value, bool) {
	if n == nil {
		return nil, false
	}
	return n.Props[key].ObjectValue()
}

// TextContent concatenates direct text children.
func (n *Node) TextContent() string {
	if n == nil {
		return ""
	}
	var builder strings.Builder
	for _, child := range n.Children {
		if child.IsText() {
			builder.WriteString(child.Text)
		}
	}
	return builder.String()
}

// FirstDescendant finds the first descendant of a type, following hoisted
// slot props as well as children.
func (n *Node) FirstDescendant(nodeType string) *Node {
	if n == nil {
		return nil
	}
	if n.Type == nodeType {
		return n
	}
	for _, child := range n.Children {
		if found := child.FirstDescendant(nodeType); found != nil {
			return found
		}
	}
	for _, value := range n.Props {
		for _, node := range value.NodesValue() {
			if found := node.FirstDescendant(nodeType); found != nil {
				return found
			}
		}
	}
	return nil
}

// Tree is the root the runtime pushes after every commit: one __screen per
// navigation stack entry.
type Tree struct {
	Screens    []*Node
	Feedback   *Node
	Navigation *Node
}

// ParseTree decodes one render JSON payload.
func ParseTree(data []byte) (*Tree, error) {
	var root map[string]any
	if err := json.Unmarshal(data, &root); err != nil {
		return nil, fmt.Errorf("decode render tree: %w", err)
	}
	rawChildren, ok := root["children"].([]any)
	if !ok {
		return nil, fmt.Errorf("render tree has no children")
	}
	tree := &Tree{}
	for _, child := range rawChildren {
		object, ok := child.(map[string]any)
		if !ok {
			continue
		}
		if node, ok := nodeFromJSON(object); ok {
			switch node.Type {
			case "__screen":
				tree.Screens = append(tree.Screens, node)
			case "FeedbackScene":
				tree.Feedback = node
			case "__navigation":
				tree.Navigation = node
			}
		}
	}
	return tree, nil
}

// Active is the screen the palette shows, the top of the stack.
func (t *Tree) Active() *Node {
	if t == nil {
		return nil
	}
	for index := len(t.Screens) - 1; index >= 0; index-- {
		if t.Screens[index].BoolProp("active") {
			return t.Screens[index]
		}
	}
	if len(t.Screens) > 0 {
		return t.Screens[len(t.Screens)-1]
	}
	return nil
}

// ActiveRoot is the active screen's single root component.
func (t *Tree) ActiveRoot() *Node {
	active := t.Active()
	if active == nil {
		return nil
	}
	for _, child := range active.Children {
		if !child.IsText() {
			return child
		}
	}
	return nil
}

// Depth is the navigation stack size, at least one.
func (t *Tree) Depth() int {
	if t == nil || len(t.Screens) == 0 {
		return 1
	}
	return len(t.Screens)
}
