//go:build windows

package windows

import (
	"context"
	"fmt"
	"runtime"
	"syscall"
	"unsafe"
)

// OLE DB is the documented Windows Search access path. The SQL is built by
// the pure file-search layer; this file only owns ABI and row iteration.
const (
	dbguidDefaultValue = 0xC8B521FB
	dbpartValue        = 0x1
	dbtypeWStr         = 130
	dbaccessorRowData  = 0x2
	maxPath            = 260
)

var (
	iidIDBInitialize    = guidValue(0x0C733A8B, 0x2A1C, 0x11CE, [8]byte{0xAD, 0xE5, 0x00, 0xAA, 0x00, 0x44, 0x77, 0x3D})
	iidIDBCreateSession = guidValue(0x0C733A5D, 0x2A1C, 0x11CE, [8]byte{0xAD, 0xE5, 0x00, 0xAA, 0x00, 0x44, 0x77, 0x3D})
	iidIDBCreateCommand = guidValue(0x0C733A1D, 0x2A1C, 0x11CE, [8]byte{0xAD, 0xE5, 0x00, 0xAA, 0x00, 0x44, 0x77, 0x3D})
	iidICommandText     = guidValue(0x0C733A63, 0x2A1C, 0x11CE, [8]byte{0xAD, 0xE5, 0x00, 0xAA, 0x00, 0x44, 0x77, 0x3D})
	iidIRowset          = guidValue(0x0C733A7C, 0x2A1C, 0x11CE, [8]byte{0xAD, 0xE5, 0x00, 0xAA, 0x00, 0x44, 0x77, 0x3D})
	iidIAccessor        = guidValue(0x0C733A8C, 0x2A1C, 0x11CE, [8]byte{0xAD, 0xE5, 0x00, 0xAA, 0x00, 0x44, 0x77, 0x3D})
	dbguidDefault       = guid{Data1: dbguidDefaultValue, Data2: 0x5CF3, Data3: 0x11CE, Data4: [8]byte{0xAD, 0xE5, 0x00, 0xAA, 0x00, 0x44, 0x77, 0x3D}}
)

// dbBinding mirrors the 64-bit DBBINDING layout.
type dbBinding struct {
	Ordinal   uintptr
	Value     uintptr
	Length    uintptr
	Status    uintptr
	TypeInfo  uintptr
	Object    uintptr
	BindExt   uintptr
	Part      uint32
	MemOwner  uint32
	ParamIO   uint32
	_         uint32
	MaxLen    uintptr
	Flags     uint32
	ValueType uint16
	Precision byte
	Scale     byte
}

// SearchIndexPaths runs one SystemIndex query and returns the displayed paths.
// Callers treat an empty result or an error as the cue to fall back to the
// bounded directory walk.
func SearchIndexPaths(ctx context.Context, sql string) ([]string, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	hr, uninit := coInitialize()
	if int32(hr) != 0 && int32(hr) != 1 && int32(hr) != rpcChangedMode {
		return nil, fmt.Errorf("CoInitializeEx: 0x%08x", uint32(hr))
	}
	if uninit {
		defer coUninitializeProc.Call()
	}
	clsid, err := clsidFromProgID("Search.CollatorDSO.1")
	if err != nil {
		return nil, err
	}
	init, err := coCreateInstance(&clsid, &iidIDBInitialize)
	if err != nil {
		return nil, err
	}
	defer comRelease(init)
	if hr := comCall(init, 3); int32(hr) != 0 { // IDBInitialize::Initialize
		return nil, fmt.Errorf("IDBInitialize.Initialize: 0x%08x", uint32(hr))
	}
	defer comCall(init, 4) // Uninitialize
	sessions := comQueryInterface(init, &iidIDBCreateSession)
	if sessions == 0 {
		return nil, fmt.Errorf("IDBCreateSession unavailable")
	}
	defer comRelease(sessions)
	var session uintptr
	if hr := comCall(sessions, 3, 0, uintptr(unsafe.Pointer(&iidIDBCreateCommand)), uintptr(unsafe.Pointer(&session))); int32(hr) != 0 || session == 0 {
		return nil, fmt.Errorf("IDBCreateSession.CreateSession: 0x%08x", uint32(hr))
	}
	defer comRelease(session)
	commands := comQueryInterface(session, &iidIDBCreateCommand)
	if commands == 0 {
		return nil, fmt.Errorf("IDBCreateCommand unavailable")
	}
	defer comRelease(commands)
	var commandUnknown uintptr
	if hr := comCall(commands, 3, 0, uintptr(unsafe.Pointer(&iidICommandText)), uintptr(unsafe.Pointer(&commandUnknown))); int32(hr) != 0 || commandUnknown == 0 {
		return nil, fmt.Errorf("IDBCreateCommand.CreateCommand: 0x%08x", uint32(hr))
	}
	defer comRelease(commandUnknown)
	command := comQueryInterface(commandUnknown, &iidICommandText)
	if command == 0 {
		return nil, fmt.Errorf("ICommandText unavailable")
	}
	defer comRelease(command)
	wide, err := syscall.UTF16FromString(sql)
	if err != nil {
		return nil, err
	}
	if hr := comCall(command, 7, uintptr(unsafe.Pointer(&dbguidDefault)), uintptr(unsafe.Pointer(&wide[0]))); int32(hr) != 0 {
		return nil, fmt.Errorf("ICommandText.SetCommandText: 0x%08x", uint32(hr))
	}
	var rowset uintptr
	if hr := comCall(command, 4, 0, uintptr(unsafe.Pointer(&iidIRowset)), 0, 0, uintptr(unsafe.Pointer(&rowset))); int32(hr) != 0 || rowset == 0 {
		return nil, fmt.Errorf("ICommand.Execute: 0x%08x", uint32(hr))
	}
	defer comRelease(rowset)
	accessorUnknown := comQueryInterface(rowset, &iidIAccessor)
	if accessorUnknown == 0 {
		return nil, fmt.Errorf("IAccessor unavailable")
	}
	defer comRelease(accessorUnknown)
	const byteSize = (maxPath + 8) * 2
	binding := dbBinding{
		Ordinal:   1,
		Part:      dbpartValue,
		MaxLen:    byteSize,
		ValueType: dbtypeWStr,
	}
	var accessor uintptr
	if hr := comCall(accessorUnknown, 4, dbaccessorRowData, 1, uintptr(unsafe.Pointer(&binding)), byteSize, uintptr(unsafe.Pointer(&accessor)), 0); int32(hr) != 0 || accessor == 0 {
		return nil, fmt.Errorf("IAccessor.CreateAccessor: 0x%08x", uint32(hr))
	}
	defer comCall(accessorUnknown, 6, accessor, 0) // ReleaseAccessor
	paths := make([]string, 0, 64)
	buffer := make([]uint16, maxPath+8)
	for {
		if err := ctx.Err(); err != nil {
			return paths, err
		}
		var obtained uintptr
		var rows *uintptr
		hr := comCall(rowset, 5, 0, 0, 1, uintptr(unsafe.Pointer(&obtained)), uintptr(unsafe.Pointer(&rows)))
		if int32(hr) < 0 || obtained == 0 || rows == nil {
			break
		}
		row := *rows
		clear(buffer)
		if hr := comCall(rowset, 4, row, accessor, uintptr(unsafe.Pointer(&buffer[0]))); int32(hr) == 0 {
			if path := syscall.UTF16ToString(buffer); path != "" {
				paths = append(paths, path)
			}
		}
		comCall(rowset, 6, obtained, uintptr(unsafe.Pointer(rows)), 0, 0, 0)
		if obtained < 1 {
			break
		}
	}
	return paths, nil
}
