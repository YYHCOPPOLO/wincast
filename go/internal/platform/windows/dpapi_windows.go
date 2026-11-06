//go:build windows

package windows

import (
	"errors"
	"fmt"
	"syscall"
	"unsafe"
)

var (
	crypt32            = syscall.NewLazyDLL("crypt32.dll")
	cryptProtectData   = crypt32.NewProc("CryptProtectData")
	cryptUnprotectData = crypt32.NewProc("CryptUnprotectData")
	localFree          = kernel32.NewProc("LocalFree")
)

const cryptProtectUIForbidden = 0x1

type dataBlob struct {
	size uint32
	data *byte
}

func blobOf(value []byte) dataBlob {
	if len(value) == 0 {
		return dataBlob{}
	}
	return dataBlob{size: uint32(len(value)), data: &value[0]}
}

// Protect encrypts plaintext for the current Windows user. Entropy is mixed
// into the key; callers must keep it stable across reads. The caller owns the
// returned bytes.
func Protect(plaintext, entropy []byte) ([]byte, error) {
	in, salt := blobOf(plaintext), blobOf(entropy)
	var out dataBlob
	ok, _, callErr := cryptProtectData.Call(
		uintptr(unsafe.Pointer(&in)), 0,
		uintptr(unsafe.Pointer(&salt)), 0, 0, cryptProtectUIForbidden,
		uintptr(unsafe.Pointer(&out)))
	if ok == 0 {
		return nil, fmt.Errorf("protect data: %w", callErr)
	}
	defer localFree.Call(uintptr(unsafe.Pointer(out.data)))
	return copiedBlob(out), nil
}

// Unprotect decrypts bytes produced by Protect for the same user and entropy.
// A wrong entropy is reported as a normal error, never as empty plaintext.
func Unprotect(ciphertext, entropy []byte) ([]byte, error) {
	if len(ciphertext) == 0 {
		return nil, errors.New("unprotect data: empty ciphertext")
	}
	in, salt := blobOf(ciphertext), blobOf(entropy)
	var out dataBlob
	ok, _, callErr := cryptUnprotectData.Call(
		uintptr(unsafe.Pointer(&in)), 0,
		uintptr(unsafe.Pointer(&salt)), 0, 0, cryptProtectUIForbidden,
		uintptr(unsafe.Pointer(&out)))
	if ok == 0 {
		return nil, fmt.Errorf("unprotect data: %w", callErr)
	}
	defer localFree.Call(uintptr(unsafe.Pointer(out.data)))
	return copiedBlob(out), nil
}

func copiedBlob(blob dataBlob) []byte {
	if blob.size == 0 || blob.data == nil {
		return nil
	}
	copied := make([]byte, blob.size)
	copy(copied, unsafe.Slice(blob.data, int(blob.size)))
	return copied
}
