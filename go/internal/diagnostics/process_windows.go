//go:build windows && amd64

package diagnostics

import (
	"fmt"
	"runtime"
	"syscall"
	"time"
	"unsafe"
)

type Snapshot struct {
	At                       time.Time
	WorkingSet, PrivateBytes uint64
	HeapBytes                uint64
	Handles                  uint32
	Goroutines               int
	CPU                      time.Duration
}

func ReadProcess() (Snapshot, error) {
	var heap runtime.MemStats
	runtime.ReadMemStats(&heap)
	snapshot := Snapshot{At: time.Now(), HeapBytes: heap.HeapAlloc, Goroutines: runtime.NumGoroutine()}
	process, err := syscall.GetCurrentProcess()
	if err != nil {
		return snapshot, err
	}
	// PROCESS_MEMORY_COUNTERS_EX on Windows/amd64.
	counters := struct {
		Size, PageFaults                                                                                                          uint32
		PeakWorkingSet, WorkingSet, QuotaPeakPaged, QuotaPaged, QuotaPeakNonPaged, QuotaNonPaged, Pagefile, PeakPagefile, Private uintptr
	}{}
	counters.Size = uint32(unsafe.Sizeof(counters))
	psapi := syscall.NewLazyDLL("psapi.dll")
	ok, _, callErr := psapi.NewProc("GetProcessMemoryInfo").Call(uintptr(process), uintptr(unsafe.Pointer(&counters)), uintptr(counters.Size))
	if ok == 0 {
		return snapshot, fmt.Errorf("read process memory: %w", callErr)
	}
	snapshot.WorkingSet, snapshot.PrivateBytes = uint64(counters.WorkingSet), uint64(counters.Private)
	kernel32 := syscall.NewLazyDLL("kernel32.dll")
	ok, _, callErr = kernel32.NewProc("GetProcessHandleCount").Call(uintptr(process), uintptr(unsafe.Pointer(&snapshot.Handles)))
	if ok == 0 {
		return snapshot, fmt.Errorf("read process handles: %w", callErr)
	}
	var created, exited, kernel, user syscall.Filetime
	if err := syscall.GetProcessTimes(process, &created, &exited, &kernel, &user); err != nil {
		return snapshot, err
	}
	ticks := (uint64(kernel.HighDateTime)<<32 | uint64(kernel.LowDateTime)) + (uint64(user.HighDateTime)<<32 | uint64(user.LowDateTime))
	snapshot.CPU = time.Duration(ticks * 100)
	return snapshot, nil
}

func MiB(bytes uint64) string { return fmt.Sprintf("%.2f MiB", float64(bytes)/(1024*1024)) }
