package backup

import (
	"bytes"
	"compress/gzip"
	"crypto/aes"
	"crypto/cipher"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"strings"

	"golang.org/x/crypto/scrypt"
)

// RaycastFormat is the container generation of a .rayconfig file.
type RaycastFormat uint8

const (
	RaycastV1 RaycastFormat = iota + 1
	RaycastV2
)

// Raycast errors keep a wrong password distinct from a malformed file.
var (
	ErrRaycastFormat = errors.New("not a Raycast export")
	ErrRaycastPass   = errors.New("wrong Raycast passphrase")
	ErrRaycastData   = errors.New("corrupt Raycast export")
)

// RaycastSnippet is one importable snippet from a Raycast payload.
type RaycastSnippet struct {
	Name    string
	Text    string
	Keyword string
}

// RaycastPayload is the supported subset of a Raycast export.
type RaycastPayload struct {
	Settings    map[string]json.RawMessage
	Snippets    []RaycastSnippet
	Unsupported []string
}

// DetectRaycast recognizes the two container generations the original app
// accepted. A v1 file is a bare AES-CBC blob whose length is a multiple of 16.
func DetectRaycast(raw []byte) RaycastFormat {
	if bytes.HasPrefix(raw, []byte("RAYCFG3\n")) {
		return RaycastV2
	}
	if len(raw) >= 32 && len(raw)%16 == 0 {
		return RaycastV1
	}
	return 0
}

// ReadRaycast decrypts and maps a .rayconfig file. The passphrase never leaves
// this function and is not kept in the returned payload.
func ReadRaycast(raw []byte, passphrase string) (*RaycastPayload, error) {
	var plain []byte
	var err error
	switch DetectRaycast(raw) {
	case RaycastV1:
		plain, err = decryptRaycastV1(raw, passphrase)
	case RaycastV2:
		plain, err = decryptRaycastV2(raw, passphrase)
	default:
		return nil, ErrRaycastFormat
	}
	if err != nil {
		return nil, err
	}
	var document any
	if err := json.Unmarshal(plain, &document); err != nil {
		return nil, ErrRaycastData
	}
	payload := &RaycastPayload{Settings: make(map[string]json.RawMessage)}
	mapped := make(map[string]json.RawMessage)
	collectRaycastSettings(document, mapped)
	payload.Settings = ApplyableSettings(mapped)
	payload.Snippets = collectRaycastSnippets(document)
	payload.Unsupported = doctorRaycastUnsupported(document)
	return payload, nil
}

func decryptRaycastV1(raw []byte, passphrase string) ([]byte, error) {
	if len(raw) < 32 || len(raw)%16 != 0 {
		return nil, ErrRaycastFormat
	}
	key := sha256.Sum256([]byte(passphrase))
	block, err := aes.NewCipher(key[:])
	if err != nil {
		return nil, ErrRaycastPass
	}
	iv := raw[:16]
	ciphertext := raw[16:]
	if len(ciphertext) == 0 || len(ciphertext)%aes.BlockSize != 0 {
		return nil, ErrRaycastPass
	}
	plain := make([]byte, len(ciphertext))
	cipher.NewCBCDecrypter(block, iv).CryptBlocks(plain, ciphertext)
	plain, err = pkcs7Unpad(plain)
	if err != nil {
		return nil, ErrRaycastPass
	}
	if len(plain) < 3 || plain[0] != 0x1f || plain[1] != 0x8b || plain[2] != 0x08 {
		return nil, ErrRaycastPass
	}
	json, err := gunzip(plain)
	if err != nil {
		return nil, ErrRaycastPass
	}
	return json, nil
}

func decryptRaycastV2(raw []byte, passphrase string) ([]byte, error) {
	const fixed, tag = 12, 16
	if !bytes.HasPrefix(raw, []byte("RAYCFG3\n")) || len(raw) < fixed+tag {
		return nil, ErrRaycastFormat
	}
	headerLength := int(uint32(raw[8]) | uint32(raw[9])<<8 | uint32(raw[10])<<16 | uint32(raw[11])<<24)
	if headerLength <= 0 || headerLength > 1<<20 || fixed+headerLength+tag > len(raw) {
		return nil, ErrRaycastData
	}
	headerJSON, err := gunzip(raw[fixed : fixed+headerLength])
	if err != nil {
		return nil, ErrRaycastData
	}
	var header struct {
		SchemaVersion int `json:"schemaVersion"`
		Encryption    struct {
			IV   string `json:"iv"`
			Salt string `json:"salt"`
		} `json:"encryption"`
	}
	if json.Unmarshal(headerJSON, &header) != nil || header.SchemaVersion != 3 {
		return nil, ErrRaycastData
	}
	iv, errIV := hex.DecodeString(header.Encryption.IV)
	salt, errSalt := hex.DecodeString(header.Encryption.Salt)
	if errIV != nil || errSalt != nil || len(iv) != 16 || len(salt) != 16 {
		return nil, ErrRaycastData
	}
	key, err := scrypt.Key([]byte(passphrase), salt, 16384, 8, 1, 32)
	if err != nil {
		return nil, ErrRaycastPass
	}
	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, ErrRaycastPass
	}
	gcm, err := cipher.NewGCMWithNonceSize(block, 16)
	if err != nil {
		return nil, ErrRaycastPass
	}
	payloadStart, payloadEnd := fixed+headerLength, len(raw)-tag
	if payloadEnd <= payloadStart {
		return nil, ErrRaycastData
	}
	sealed := make([]byte, 0, payloadEnd-payloadStart+tag)
	sealed = append(sealed, raw[payloadStart:payloadEnd]...)
	sealed = append(sealed, raw[payloadEnd:]...)
	gzipPayload, err := gcm.Open(nil, iv, sealed, nil)
	if err != nil {
		return nil, ErrRaycastPass
	}
	json, err := gunzip(gzipPayload)
	if err != nil {
		return nil, ErrRaycastData
	}
	return json, nil
}

func pkcs7Unpad(data []byte) ([]byte, error) {
	if len(data) == 0 {
		return nil, errors.New("empty plaintext")
	}
	padding := int(data[len(data)-1])
	if padding == 0 || padding > aes.BlockSize || padding > len(data) {
		return nil, errors.New("invalid padding")
	}
	for _, value := range data[len(data)-padding:] {
		if int(value) != padding {
			return nil, errors.New("invalid padding")
		}
	}
	return data[:len(data)-padding], nil
}

func gunzip(data []byte) ([]byte, error) {
	reader, err := gzip.NewReader(bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	defer reader.Close()
	output, err := io.ReadAll(io.LimitReader(reader, 32<<20))
	if err != nil {
		return nil, err
	}
	if len(output) == 0 {
		return nil, errors.New("empty gzip payload")
	}
	return output, nil
}

// collectRaycastSettings walks the payload because Raycast nests the same
// preference under different containers in v1 and v2 exports.
func collectRaycastSettings(value any, out map[string]json.RawMessage) {
	switch typed := value.(type) {
	case map[string]any:
		for key, child := range typed {
			switch key {
			case "compactMode", "showFavoritesInCompactMode", "showInMenuBar",
				"emojiSkinTone", "popToRootTimeout", "clipboardDisabledApps",
				"hyperKeyPhysicalKey", "hyperKeyIncludesShift", "fileSearchEnabled",
				"fileSearchScopes", "notesEnabled":
				storeRaycastSetting(key, child, out)
			case "raycastPreferredWindowMode":
				if mode, ok := child.(string); ok {
					storeRaycastSetting("compactMode", mode == "compact", out)
				}
			case "statusBarIsVisible":
				storeRaycastSetting("showInMenuBar", child, out)
			case "clipboardDisabledApplications":
				storeRaycastSetting("clipboardDisabledApps", child, out)
			}
			if key == "settings" || key == "preferences" || key == "advanced" ||
				key == "appearance" || key == "builtin_package_raycastPreferences" {
				collectRaycastSettings(child, out)
				continue
			}
			collectRaycastSettings(child, out)
		}
	case []any:
		for _, child := range typed {
			collectRaycastSettings(child, out)
		}
	}
}

func storeRaycastSetting(key string, value any, out map[string]json.RawMessage) {
	encoded, err := json.Marshal(value)
	if err != nil || len(encoded) == 0 {
		return
	}
	switch key {
	case "emojiSkinTone":
		if _, ok := value.(string); !ok {
			return
		}
	case "clipboardDisabledApps", "fileSearchScopes":
		if _, ok := value.([]any); !ok {
			return
		}
	case "popToRootTimeout":
		if _, ok := value.(float64); !ok {
			return
		}
	default:
		if _, ok := value.(bool); !ok {
			return
		}
	}
	if _, exists := out[key]; !exists {
		out[key] = encoded
	}
}

func collectRaycastSnippets(value any) []RaycastSnippet {
	var snippets []RaycastSnippet
	var walk func(any)
	walk = func(node any) {
		switch typed := node.(type) {
		case map[string]any:
			for key, child := range typed {
				lower := strings.ToLower(key)
				if strings.Contains(lower, "snippet") {
					snippets = append(snippets, parseRaycastSnippetList(child)...)
				}
				walk(child)
			}
		case []any:
			for _, child := range typed {
				walk(child)
			}
		}
	}
	walk(value)
	return dedupeRaycastSnippets(snippets)
}

func parseRaycastSnippetList(value any) []RaycastSnippet {
	var snippets []RaycastSnippet
	add := func(node any) {
		record, ok := node.(map[string]any)
		if !ok {
			return
		}
		name, _ := record["name"].(string)
		text, _ := record["text"].(string)
		if strings.TrimSpace(name) == "" || strings.TrimSpace(text) == "" {
			return
		}
		keyword, _ := record["keyword"].(string)
		snippets = append(snippets, RaycastSnippet{Name: name, Text: text, Keyword: keyword})
	}
	switch typed := value.(type) {
	case []any:
		for _, child := range typed {
			add(child)
		}
	case map[string]any:
		add(typed)
		for _, child := range typed {
			if list, ok := child.([]any); ok {
				for _, entry := range list {
					add(entry)
				}
			}
		}
	}
	return snippets
}

func dedupeRaycastSnippets(snippets []RaycastSnippet) []RaycastSnippet {
	seen := make(map[string]bool, len(snippets))
	result := make([]RaycastSnippet, 0, len(snippets))
	for _, snippet := range snippets {
		key := snippet.Name + "\x00" + snippet.Text
		if seen[key] {
			continue
		}
		seen[key] = true
		result = append(result, snippet)
	}
	return result
}

func doctorRaycastUnsupported(value any) []string {
	var unsupported []string
	seen := make(map[string]bool)
	var walk func(any)
	walk = func(node any) {
		switch typed := node.(type) {
		case map[string]any:
			for key, child := range typed {
				lower := strings.ToLower(key)
				switch {
				case strings.Contains(lower, "clipboard") && isList(child):
					if !seen["clipboard history"] {
						seen["clipboard history"] = true
						unsupported = append(unsupported, "clipboard history")
					}
				case strings.Contains(lower, "favorite") && isList(child):
					if !seen["favorites"] {
						seen["favorites"] = true
						unsupported = append(unsupported, "favorites (application IDs differ)")
					}
				case strings.Contains(lower, "hotkey") && child != nil:
					if !seen["hotkeys"] {
						seen["hotkeys"] = true
						unsupported = append(unsupported, "keyboard shortcuts (Carbon key codes)")
					}
				}
				walk(child)
			}
		case []any:
			for _, child := range typed {
				walk(child)
			}
		}
	}
	walk(value)
	return unsupported
}

func isList(value any) bool {
	switch typed := value.(type) {
	case []any:
		return len(typed) > 0
	case map[string]any:
		return len(typed) > 0
	}
	return false
}
