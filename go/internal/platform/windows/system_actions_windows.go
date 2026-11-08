//go:build windows

package windows

import (
	"errors"
	"fmt"
	"strings"
	"sync"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows/registry"
)

var (
	shell32  = syscall.NewLazyDLL("shell32.dll")
	advapi32 = syscall.NewLazyDLL("advapi32.dll")
	powrprof = syscall.NewLazyDLL("powrprof.dll")
	setupapi = syscall.NewLazyDLL("setupapi.dll")
	cfgmgr32 = syscall.NewLazyDLL("cfgmgr32.dll")

	lockWorkStation      = user32.NewProc("LockWorkStation")
	exitWindowsEx        = user32.NewProc("ExitWindowsEx")
	sendMessageTimeout   = user32.NewProc("SendMessageTimeoutW")
	enumWindows          = user32.NewProc("EnumWindows")
	isWindowVisible      = user32.NewProc("IsWindowVisible")
	getWindow            = user32.NewProc("GetWindow")
	getWindowThreadPID   = user32.NewProc("GetWindowThreadProcessId")
	getWindowTextLength  = user32.NewProc("GetWindowTextLengthW")
	getClassNameW        = user32.NewProc("GetClassNameW")
	getWindowLongW       = user32.NewProc("GetWindowLongW")
	shellExecuteW        = shell32.NewProc("ShellExecuteW")
	shQueryRecycleBin    = shell32.NewProc("SHQueryRecycleBinW")
	shEmptyRecycleBin    = shell32.NewProc("SHEmptyRecycleBinW")
	shChangeNotify       = shell32.NewProc("SHChangeNotify")
	setSuspendState      = powrprof.NewProc("SetSuspendState")
	openProcessToken     = advapi32.NewProc("OpenProcessToken")
	lookupPrivilegeValue = advapi32.NewProc("LookupPrivilegeValueW")
	adjustTokenPrivs     = advapi32.NewProc("AdjustTokenPrivileges")
	setupDiGetClassDevs  = setupapi.NewProc("SetupDiGetClassDevsW")
	setupDiEnumDevice    = setupapi.NewProc("SetupDiEnumDeviceInfo")
	setupDiGetProperty   = setupapi.NewProc("SetupDiGetDeviceRegistryPropertyW")
	setupDiDestroy       = setupapi.NewProc("SetupDiDestroyDeviceInfoList")
	cmRequestDeviceEject = cfgmgr32.NewProc("CM_Request_Device_EjectW")

	guidDiskDrive = guidValue(0x4D36E967, 0xE325, 0x11CE, [8]byte{0xBF, 0xC1, 0x08, 0x00, 0x2B, 0xE1, 0x03, 0x18})

	enumMu        sync.Mutex
	enumPIDs      []uint32
	enumHandles   []uintptr
	enumMode      int
	enumOwnPID    uint32
	enumPrevPID   uint32
	hiddenWindows []uintptr
)

const (
	swHide          = 0
	swShowNA        = 8
	wmSetting       = 0x001A
	wmSysCommand    = 0x0112
	scMonitorPower  = 0xF170
	scScreenSaver   = 0xF140
	smtoAbortIfHung = 0x0002
	hwndBroadcast   = 0xFFFF
	exitLogoff      = 0x0
	exitShutdown    = 0x1
	exitReboot      = 0x2
	exitForceIfHung = 0x10
	shutdownReason  = 0x80000000
	vkMediaNext     = 0xB0
	vkMediaPrev     = 0xB1
	vkMediaPlay     = 0xB3
	vkVolumeMute    = 0xAD
	vkVolumeDown    = 0xAE
	vkVolumeUp      = 0xAF
	vkLeftWin       = 0x5B
)

// SystemActionError carries an optional Windows Settings URI for failures
// that the user can resolve there.
type SystemActionError struct {
	Message     string
	SettingsURI string
}

func (e *SystemActionError) Error() string { return e.Message }

// SystemActionResult is the user-facing outcome of one action. Message is
// empty for silent actions.
type SystemActionResult struct {
	Message string
}

// ExecuteSystemAction performs one catalog action. Callers gate confirmation
// before calling this function.
func ExecuteSystemAction(raw string) (SystemActionResult, error) {
	switch raw {
	case "lock-screen":
		if ok, _, err := lockWorkStation.Call(); ok == 0 {
			return SystemActionResult{}, fmt.Errorf("Windows could not lock the workstation: %v", err)
		}
		return SystemActionResult{}, nil
	case "sleep":
		if ok, _, _ := setSuspendState.Call(0, 0, 0); ok == 0 {
			return SystemActionResult{}, errors.New("Windows could not put the PC to sleep")
		}
		return SystemActionResult{}, nil
	case "sleep-displays":
		sysCommand(scMonitorPower, 2)
		return SystemActionResult{}, nil
	case "show-screen-saver":
		sysCommand(scScreenSaver, 0)
		return SystemActionResult{}, nil
	case "restart":
		return SystemActionResult{}, sessionEnd(exitReboot | exitForceIfHung)
	case "shut-down":
		return SystemActionResult{}, sessionEnd(exitShutdown | exitForceIfHung)
	case "log-out":
		return SystemActionResult{}, sessionEnd(exitLogoff)
	case "play-pause":
		return SystemActionResult{}, sendVirtualKey(vkMediaPlay)
	case "next-track":
		return SystemActionResult{}, sendVirtualKey(vkMediaNext)
	case "previous-track":
		return SystemActionResult{}, sendVirtualKey(vkMediaPrev)
	case "toggle-mute":
		if err := ToggleMute(); err != nil {
			return SystemActionResult{}, sendVirtualKey(vkVolumeMute)
		}
		return SystemActionResult{}, nil
	case "volume-up":
		if err := StepVolume(true); err != nil {
			return SystemActionResult{}, sendVirtualKey(vkVolumeUp)
		}
		return SystemActionResult{}, nil
	case "volume-down":
		if err := StepVolume(false); err != nil {
			return SystemActionResult{}, sendVirtualKey(vkVolumeDown)
		}
		return SystemActionResult{}, nil
	case "volume-0":
		return SystemActionResult{}, SetVolume(0)
	case "volume-25":
		return SystemActionResult{}, SetVolume(0.25)
	case "volume-50":
		return SystemActionResult{}, SetVolume(0.50)
	case "volume-75":
		return SystemActionResult{}, SetVolume(0.75)
	case "volume-100":
		return SystemActionResult{}, SetVolume(1)
	case "show-desktop":
		return SystemActionResult{}, showDesktop()
	case "toggle-system-appearance":
		return toggleAppearance()
	case "toggle-stage-manager":
		return SystemActionResult{Message: "Stage Manager is not available on Windows."}, nil
	case "open-trash":
		return SystemActionResult{}, shellOpen("shell:RecycleBinFolder")
	case "empty-trash":
		return emptyTrash()
	case "eject-all-disks":
		return ejectAllDisks()
	case "toggle-hidden-files":
		return toggleHiddenFiles()
	case "hide-all-apps-except-frontmost":
		return SystemActionResult{}, hideOtherApps(0)
	case "unhide-all-hidden-apps":
		return unhideAllApps()
	case "quit-all-apps":
		quitAllApps()
		return SystemActionResult{}, nil
	case "dismiss-notifications":
		return SystemActionResult{Message: "Dismissing notifications is not available on Windows."}, nil
	case "toggle-bluetooth":
		return toggleBluetoothRadio()
	}
	return SystemActionResult{}, fmt.Errorf("unknown system action %q", raw)
}

// QuitAllTargets lists the PIDs a Quit All action would ask to close.
func QuitAllTargets() []uint32 {
	collectWindows(enumQuit)
	return uniquePIDs(enumPIDs)
}

func sysCommand(command, lparam uintptr) {
	var result uintptr
	sendMessageTimeout.Call(hwndBroadcast, wmSysCommand, command, lparam, smtoAbortIfHung, 1000, uintptr(unsafe.Pointer(&result)))
}

func sessionEnd(flags uintptr) error {
	enableShutdownPrivilege()
	ok, _, err := exitWindowsEx.Call(flags, shutdownReason)
	if ok == 0 {
		return fmt.Errorf("Windows could not end the session: %v", err)
	}
	return nil
}

type luid struct {
	Low  uint32
	High int32
}

type tokenPrivileges struct {
	Count      uint32
	_          uint32
	LuidLow    uint32
	LuidHigh   int32
	Attributes uint32
	_          uint32
}

func enableShutdownPrivilege() {
	process, _, _ := syscall.NewLazyDLL("kernel32.dll").NewProc("GetCurrentProcess").Call()
	var token uintptr
	if ok, _, _ := openProcessToken.Call(process, 0x28, uintptr(unsafe.Pointer(&token))); ok == 0 {
		return
	}
	defer syscall.CloseHandle(syscall.Handle(token))
	name, _ := syscall.UTF16PtrFromString("SeShutdownPrivilege")
	var id luid
	if ok, _, _ := lookupPrivilegeValue.Call(0, uintptr(unsafe.Pointer(name)), uintptr(unsafe.Pointer(&id))); ok == 0 {
		return
	}
	privileges := tokenPrivileges{Count: 1, LuidLow: id.Low, LuidHigh: id.High, Attributes: 0x2}
	adjustTokenPrivs.Call(token, 0, uintptr(unsafe.Pointer(&privileges)), 0, 0, 0)
}

func showDesktop() error {
	inputs := []keyboardInput{
		taggedKeyInput(vkLeftWin, 0, 0),
		taggedKeyInput('D', 0, 0),
		taggedKeyInput('D', 0, keyEventKeyUp),
		taggedKeyInput(vkLeftWin, 0, keyEventKeyUp),
	}
	return sendInputs(inputs)
}

func shellOpen(target string) error {
	wide, err := syscall.UTF16PtrFromString(target)
	if err != nil {
		return err
	}
	result, _, _ := shellExecuteW.Call(0, 0, uintptr(unsafe.Pointer(wide)), 0, 0, 1)
	if result <= 32 {
		return fmt.Errorf("Explorer could not open %s", target)
	}
	return nil
}

type shQueryRBInfo struct {
	CbSize   uint32
	_        uint32
	Size     int64
	NumItems int64
}

func emptyTrash() (SystemActionResult, error) {
	info := shQueryRBInfo{CbSize: 24}
	shQueryRecycleBin.Call(0, uintptr(unsafe.Pointer(&info)))
	if info.NumItems <= 0 {
		return SystemActionResult{Message: "Trash Is Already Empty"}, nil
	}
	result, _, _ := shEmptyRecycleBin.Call(0, 0, 0x1|0x2|0x4)
	if result != 0 {
		return SystemActionResult{}, errors.New("Windows could not empty the Recycle Bin")
	}
	return SystemActionResult{Message: "Trash Emptied"}, nil
}

func toggleAppearance() (SystemActionResult, error) {
	key, err := registry.OpenKey(registry.CURRENT_USER,
		`Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`, registry.QUERY_VALUE|registry.SET_VALUE)
	if err != nil {
		return SystemActionResult{}, errors.New("Windows could not read the current appearance")
	}
	defer key.Close()
	light, _, err := key.GetIntegerValue("AppsUseLightTheme")
	if err != nil {
		return SystemActionResult{}, errors.New("Windows could not read the current appearance")
	}
	next := uint32(0)
	message := "Dark Appearance"
	if light == 0 {
		next = 1
		message = "Light Appearance"
	}
	if err := key.SetDWordValue("AppsUseLightTheme", next); err != nil {
		return SystemActionResult{}, err
	}
	if err := key.SetDWordValue("SystemUsesLightTheme", next); err != nil {
		return SystemActionResult{}, err
	}
	broadcastSetting("ImmersiveColorSet")
	return SystemActionResult{Message: message}, nil
}

func toggleHiddenFiles() (SystemActionResult, error) {
	key, err := registry.OpenKey(registry.CURRENT_USER,
		`Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced`, registry.QUERY_VALUE|registry.SET_VALUE)
	if err != nil {
		return SystemActionResult{}, errors.New("Windows could not read the Explorer setting")
	}
	defer key.Close()
	current, _, err := key.GetIntegerValue("Hidden")
	if err != nil {
		current = 2
	}
	show := current != 1
	next := uint32(2)
	if show {
		next = 1
	}
	if err := key.SetDWordValue("Hidden", next); err != nil {
		return SystemActionResult{}, err
	}
	shChangeNotify.Call(0x08000000, 0, 0, 0)
	broadcastSetting("windows")
	if show {
		return SystemActionResult{Message: "Hidden Files Shown"}, nil
	}
	return SystemActionResult{Message: "Hidden Files Hidden"}, nil
}

func broadcastSetting(name string) {
	wide, err := syscall.UTF16PtrFromString(name)
	if err != nil {
		return
	}
	var result uintptr
	sendMessageTimeout.Call(hwndBroadcast, wmSetting, 0, uintptr(unsafe.Pointer(wide)), smtoAbortIfHung, 1000, uintptr(unsafe.Pointer(&result)))
}

type spDeviceInfoData struct {
	CbSize    uint32
	_         uint32
	ClassGuid guid
	DevInst   uint32
	_         uint32
	Reserved  uintptr
}

func ejectAllDisks() (SystemActionResult, error) {
	handle, _, err := setupDiGetClassDevs.Call(uintptr(unsafe.Pointer(&guidDiskDrive)), 0, 0, 0x2)
	if handle == 0 || handle == ^uintptr(0) {
		return SystemActionResult{}, fmt.Errorf("Windows could not list removable disks: %v", err)
	}
	defer setupDiDestroy.Call(handle)
	ejected := 0
	info := spDeviceInfoData{CbSize: uint32(unsafe.Sizeof(spDeviceInfoData{}))}
	for index := uint32(0); ; index++ {
		if ok, _, _ := setupDiEnumDevice.Call(handle, uintptr(index), uintptr(unsafe.Pointer(&info))); ok == 0 {
			break
		}
		var policy uint32
		var regType, needed uint32
		ok, _, _ := setupDiGetProperty.Call(
			handle, uintptr(unsafe.Pointer(&info)), 0x1F,
			uintptr(unsafe.Pointer(&regType)),
			uintptr(unsafe.Pointer(&policy)), uintptr(unsafe.Sizeof(policy)),
			uintptr(unsafe.Pointer(&needed)),
		)
		if ok == 0 || policy == 1 {
			continue
		}
		if result, _, _ := cmRequestDeviceEject.Call(uintptr(info.DevInst), 0, 0, 0); result == 0 {
			ejected++
		}
	}
	if ejected == 0 {
		return SystemActionResult{Message: "No Disks to Eject"}, nil
	}
	if ejected == 1 {
		return SystemActionResult{Message: "1 Disk Ejected"}, nil
	}
	return SystemActionResult{Message: fmt.Sprintf("%d Disks Ejected", ejected)}, nil
}

const (
	enumQuit = 1
	enumHide = 2
)

func collectWindows(mode int) {
	enumMu.Lock()
	enumPIDs = nil
	enumHandles = nil
	enumMode = mode
	enumOwnPID = uint32(syscall.Getpid())
	enumPrevPID = 0
	enumMu.Unlock()
	enumWindows.Call(syscall.NewCallback(enumWindowCallback), 0)
}

func enumWindowCallback(hwnd uintptr, _ uintptr) uintptr {
	if ok, _, _ := isWindowVisible.Call(hwnd); ok == 0 {
		return 1
	}
	if owner, _, _ := getWindow.Call(hwnd, 4); owner != 0 {
		return 1
	}
	if isShellWindow(hwnd) {
		return 1
	}
	var pid uint32
	getWindowThreadPID.Call(hwnd, uintptr(unsafe.Pointer(&pid)))
	enumMu.Lock()
	own, mode := enumOwnPID, enumMode
	enumMu.Unlock()
	if pid == 0 || pid == own || isExplorerProcess(pid) {
		return 1
	}
	switch mode {
	case enumQuit:
		if length, _, _ := getWindowTextLength.Call(hwnd); length == 0 {
			return 1
		}
		enumMu.Lock()
		enumPIDs = append(enumPIDs, pid)
		enumMu.Unlock()
	case enumHide:
		style, _, _ := getWindowLongW.Call(hwnd, 0xFFFFFFEC)
		if style&0x80 != 0 && style&0x40000 == 0 { // WS_EX_TOOLWINDOW without WS_EX_APPWINDOW
			return 1
		}
		enumMu.Lock()
		enumHandles = append(enumHandles, hwnd)
		enumMu.Unlock()
	}
	return 1
}

func isShellWindow(hwnd uintptr) bool {
	var buffer [64]uint16
	length, _, _ := getClassNameW.Call(hwnd, uintptr(unsafe.Pointer(&buffer[0])), uintptr(len(buffer)))
	if length == 0 {
		return false
	}
	switch syscall.UTF16ToString(buffer[:length]) {
	case "Shell_TrayWnd", "Shell_SecondaryTrayWnd", "Progman", "WorkerW", "NotifyIconOverflowWindow":
		return true
	}
	return false
}

func isExplorerProcess(pid uint32) bool {
	handle, err := syscall.OpenProcess(0x1000, false, pid)
	if err != nil {
		return false
	}
	defer syscall.CloseHandle(handle)
	queryName := kernel32.NewProc("QueryFullProcessImageNameW")
	var buffer [1024]uint16
	size := uint32(len(buffer))
	if ok, _, _ := queryName.Call(uintptr(handle), 0, uintptr(unsafe.Pointer(&buffer[0])), uintptr(unsafe.Pointer(&size))); ok == 0 {
		return false
	}
	return strings.HasSuffix(strings.ToLower(syscall.UTF16ToString(buffer[:size])), `\explorer.exe`)
}

func uniquePIDs(pids []uint32) []uint32 {
	seen := make(map[uint32]bool, len(pids))
	out := make([]uint32, 0, len(pids))
	for _, pid := range pids {
		if !seen[pid] {
			seen[pid] = true
			out = append(out, pid)
		}
	}
	return out
}

func hideOtherApps(previousPID uint32) error {
	enumMu.Lock()
	hiddenWindows = nil
	enumPrevPID = previousPID
	enumMu.Unlock()
	collectWindows(enumHide)
	enumMu.Lock()
	handles := append([]uintptr(nil), enumHandles...)
	hiddenWindows = append([]uintptr(nil), handles...)
	enumMu.Unlock()
	for _, hwnd := range handles {
		showWindow.Call(hwnd, swHide)
	}
	return nil
}

func unhideAllApps() (SystemActionResult, error) {
	enumMu.Lock()
	handles := hiddenWindows
	hiddenWindows = nil
	enumMu.Unlock()
	if len(handles) == 0 {
		return SystemActionResult{Message: "Nothing Was Hidden"}, nil
	}
	for _, hwnd := range handles {
		showWindow.Call(hwnd, swShowNA)
	}
	return SystemActionResult{Message: "All Apps Unhidden"}, nil
}

func quitAllApps() {
	collectWindows(enumQuit)
	enumMu.Lock()
	handles := append([]uintptr(nil), enumHandles...)
	pids := append([]uint32(nil), enumPIDs...)
	enumMu.Unlock()
	_ = handles
	for _, pid := range uniquePIDs(pids) {
		enumMu.Lock()
		enumPrevPID = pid
		enumMu.Unlock()
		closePIDWindows(pid)
	}
}

func closePIDWindows(pid uint32) {
	enumMu.Lock()
	enumMode = 0
	enumPrevPID = pid
	enumMu.Unlock()
	closeProc := syscall.NewCallback(func(hwnd uintptr, _ uintptr) uintptr {
		if visible, _, _ := isWindowVisible.Call(hwnd); visible == 0 {
			return 1
		}
		var windowPID uint32
		getWindowThreadPID.Call(hwnd, uintptr(unsafe.Pointer(&windowPID)))
		if windowPID == pid {
			if owner, _, _ := getWindow.Call(hwnd, 4); owner == 0 {
				postMessage.Call(hwnd, wmClose, 0, 0)
			}
		}
		return 1
	})
	enumWindows.Call(closeProc, 0)
}

// toggleBluetoothRadio is completed by the WinRT radio bridge. Until then the
// action reports a real permission/settings failure instead of pretending to
// have toggled the radio.
func toggleBluetoothRadio() (SystemActionResult, error) {
	message := "Bluetooth control requires the Windows radio access prompt."
	if err := requestBluetoothAccess(); err == nil {
		if state, err := toggleBluetoothState(); err == nil {
			if state {
				return SystemActionResult{Message: "Bluetooth On"}, nil
			}
			return SystemActionResult{Message: "Bluetooth Off"}, nil
		}
	}
	return SystemActionResult{}, &SystemActionError{Message: message, SettingsURI: "ms-settings:bluetooth"}
}
