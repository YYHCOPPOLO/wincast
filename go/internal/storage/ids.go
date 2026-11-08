package storage

import (
	"crypto/rand"
	"encoding/hex"
)

// NewID returns a random RFC 4122 version 4 identifier in its canonical text
// form. Business identity is always this text; rowids and list positions are
// never identifiers.
func NewID() (string, error) {
	var raw [16]byte
	if _, err := rand.Read(raw[:]); err != nil {
		return "", err
	}
	raw[6] = raw[6]&0x0f | 0x40
	raw[8] = raw[8]&0x3f | 0x80
	dst := make([]byte, 36)
	hex.Encode(dst[0:8], raw[0:4])
	dst[8] = '-'
	hex.Encode(dst[9:13], raw[4:6])
	dst[13] = '-'
	hex.Encode(dst[14:18], raw[6:8])
	dst[18] = '-'
	hex.Encode(dst[19:23], raw[8:10])
	dst[23] = '-'
	hex.Encode(dst[24:36], raw[10:16])
	return string(dst), nil
}

// ValidID reports whether text is a canonical lowercase UUID v4. Stored
// identifiers from earlier application versions use this shape.
func ValidID(text string) bool {
	if len(text) != 36 {
		return false
	}
	for i, r := range text {
		switch i {
		case 8, 13, 18, 23:
			if r != '-' {
				return false
			}
		default:
			if (r < '0' || r > '9') && (r < 'a' || r > 'f') {
				return false
			}
		}
	}
	return text[14] == '4' && (text[19] == '8' || text[19] == '9' || text[19] == 'a' || text[19] == 'b')
}
