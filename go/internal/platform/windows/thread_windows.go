//go:build windows

package windows

import (
	"errors"
	"fmt"
	"runtime"
	"sync"
	"sync/atomic"
	"syscall"
	"unsafe"
)

var (
	registerClassEx           = user32.NewProc("RegisterClassExW")
	createWindowEx            = user32.NewProc("CreateWindowExW")
	destroyWindow             = user32.NewProc("DestroyWindow")
	defWindowProc             = user32.NewProc("DefWindowProcW")
	getMessage                = user32.NewProc("GetMessageW")
	translateMessage          = user32.NewProc("TranslateMessage")
	dispatchMessage           = user32.NewProc("DispatchMessageW")
	postMessage               = user32.NewProc("PostMessageW")
	postQuitMessage           = user32.NewProc("PostQuitMessage")
	addClipboardFormat        = user32.NewProc("AddClipboardFormatListener")
	removeClipboardFormat     = user32.NewProc("RemoveClipboardFormatListener")
	setWindowsHookEx          = user32.NewProc("SetWindowsHookExW")
	callNextHookEx            = user32.NewProc("CallNextHookEx")
	unhookWindowsHookEx       = user32.NewProc("UnhookWindowsHookEx")
	getModuleHandle           = kernel32.NewProc("GetModuleHandleW")
	moveMemory                = syscall.NewLazyDLL("ntdll.dll").NewProc("RtlMoveMemory")
	coInitializeEx            = syscall.NewLazyDLL("ole32.dll").NewProc("CoInitializeEx")
	coUninitialize            = syscall.NewLazyDLL("ole32.dll").NewProc("CoUninitialize")
	messageHostSequence       atomic.Uint64
	messageHosts              sync.Map // HWND -> *MessageHost
	messageHostDispatchWindow = uint32(0x8000 + 1)
	messageHostProc           = syscall.NewCallback(messageHostWndProc)
	keyboardHookProc          = syscall.NewCallback(keyboardHookCallback)
	keyboardHook              struct {
		mu       sync.Mutex
		callback func(KeyEvent)
	}
)

// KeyEvent is one low-level keyboard transition. Injected events are
// reported so callers never treat automation as a user gesture.
type KeyEvent struct {
	VirtualKey uint32
	Down       bool
	Injected   bool
}

type keyboardHookInfo struct {
	virtualKey uint32
	scanCode   uint32
	flags      uint32
	time       uint32
	extra      uintptr
}

const (
	wmDestroy                    = 0x0002
	wmClose                      = 0x0010
	wmNCCreate                   = 0x0081
	wmNCDestroy                  = 0x0082
	wmSettingChange              = 0x001A
	wmClipboardUpdate            = 0x031D
	wmColorizationChanged        = 0x0320
	wmKeyDown                    = 0x0100
	wmKeyUp                      = 0x0101
	wmSysKeyDown                 = 0x0104
	wmSysKeyUp                   = 0x0105
	whKeyboardLL          uint32 = 13
	llkhfInjected         uint32 = 0x10
	coinitApartmentThread        = 0x2
	rpcChangedMode        int32  = -2147417850 // 0x80010106
)

type windowMessage struct {
	window  uintptr
	message uint32
	_       uint32
	wParam  uintptr
	lParam  uintptr
	time    uint32
	x, y    int32
	private uint32
}

type windowClassEx struct {
	size, style     uint32
	windowProc      uintptr
	classExtra      int32
	windowExtra     int32
	instance        syscall.Handle
	icon, cursor    syscall.Handle
	background      syscall.Handle
	menuName, class *uint16
	iconSmall       syscall.Handle
}

// MessageHost owns one hidden top-level window and its Win32 message loop on a
// dedicated OS thread. It is the home for hooks, clipboard notifications and
// system broadcasts that must not block MyGo's UI loop. Callbacks run on this
// thread; callers that touch UI state must marshal their work back to MyGo.
type MessageHost struct {
	hwnd     uintptr
	ready    chan struct{}
	done     chan struct{}
	startErr error
	close    sync.Once

	mu          sync.Mutex
	tasks       []func()
	onClipboard func()
	onTheme     func(dark bool)
	hook        uintptr
}

func StartMessageHost() (*MessageHost, error) {
	host := &MessageHost{ready: make(chan struct{}), done: make(chan struct{})}
	go host.run()
	<-host.ready
	if host.startErr != nil {
		return nil, host.startErr
	}
	return host, nil
}

func (h *MessageHost) run() {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	defer close(h.done)
	instance, _, _ := getModuleHandle.Call(0)
	name, err := syscall.UTF16PtrFromString(fmt.Sprintf("TinycastMessageHost%d", messageHostSequence.Add(1)))
	if err != nil {
		h.startErr = err
		close(h.ready)
		return
	}
	class := windowClassEx{
		style:      0x0003, // CS_HREDRAW | CS_VREDRAW
		windowProc: messageHostProc,
		instance:   syscall.Handle(instance),
		class:      name,
	}
	class.size = uint32(unsafe.Sizeof(class))
	atom, _, callErr := registerClassEx.Call(uintptr(unsafe.Pointer(&class)))
	if atom == 0 {
		h.startErr = fmt.Errorf("register message host window: %w", callErr)
		close(h.ready)
		return
	}
	hwnd, _, callErr := createWindowEx.Call(0, uintptr(unsafe.Pointer(name)), 0, 0x80000000, /* WS_POPUP */
		0, 0, 0, 0, 0, 0, instance, 0)
	if hwnd == 0 {
		h.startErr = fmt.Errorf("create message host window: %w", callErr)
		close(h.ready)
		return
	}
	h.hwnd = hwnd
	messageHosts.Store(hwnd, h)
	h.startErr = nil
	close(h.ready)
	for {
		var message windowMessage
		ok, _, _ := getMessage.Call(uintptr(unsafe.Pointer(&message)), 0, 0, 0)
		if int32(ok) <= 0 {
			break
		}
		translateMessage.Call(uintptr(unsafe.Pointer(&message)))
		dispatchMessage.Call(uintptr(unsafe.Pointer(&message)))
	}
	messageHosts.Delete(hwnd)
}

// Post schedules fn on the host thread. It returns false after Close.
func (h *MessageHost) Post(fn func()) bool {
	if fn == nil {
		return false
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	if h.hwnd == 0 {
		return false
	}
	h.tasks = append(h.tasks, fn)
	ok, _, _ := postMessage.Call(h.hwnd, uintptr(messageHostDispatchWindow), 0, 0)
	if ok == 0 {
		h.tasks = h.tasks[:len(h.tasks)-1]
		return false
	}
	return true
}

func (h *MessageHost) drain() {
	h.mu.Lock()
	tasks := h.tasks
	h.tasks = nil
	h.mu.Unlock()
	for _, task := range tasks {
		task()
	}
}

// WatchClipboard registers for WM_CLIPBOARDUPDATE. The callback runs on the
// host thread and must only enqueue work, never read clipboard content itself.
func (h *MessageHost) WatchClipboard(callback func()) error {
	h.mu.Lock()
	h.onClipboard = callback
	h.mu.Unlock()
	ok, _, callErr := addClipboardFormat.Call(h.hwnd)
	if ok == 0 {
		return fmt.Errorf("listen for clipboard updates: %w", callErr)
	}
	return nil
}

// WatchTheme reports system appearance changes (WM_SETTINGCHANGE and DWM
// colorization). The callback runs on the host thread.
func (h *MessageHost) WatchTheme(callback func(dark bool)) error {
	h.mu.Lock()
	h.onTheme = callback
	h.mu.Unlock()
	return nil
}

// MonitorKeyboard installs one low-level keyboard hook on the host thread.
// The callback runs there and must not block; it never consumes a key.
func (h *MessageHost) MonitorKeyboard(callback func(KeyEvent)) error {
	result := make(chan error, 1)
	if !h.Post(func() { result <- h.installKeyboard(callback) }) {
		return errors.New("message host is closed")
	}
	return <-result
}

func (h *MessageHost) installKeyboard(callback func(KeyEvent)) error {
	h.mu.Lock()
	defer h.mu.Unlock()
	if h.hook != 0 {
		return errors.New("keyboard hook is already installed")
	}
	module, _, _ := getModuleHandle.Call(0)
	hook, _, callErr := setWindowsHookEx.Call(uintptr(whKeyboardLL), keyboardHookProc, module, 0)
	if hook == 0 {
		return fmt.Errorf("install keyboard hook: %w", callErr)
	}
	h.hook = hook
	keyboardHook.mu.Lock()
	keyboardHook.callback = callback
	keyboardHook.mu.Unlock()
	return nil
}

// StopKeyboard removes the low-level hook. Safe when none is installed.
func (h *MessageHost) StopKeyboard() {
	done := make(chan struct{})
	if !h.Post(func() { h.removeKeyboard(); close(done) }) {
		return
	}
	<-done
}

func (h *MessageHost) removeKeyboard() {
	h.mu.Lock()
	hook := h.hook
	h.hook = 0
	h.mu.Unlock()
	if hook == 0 {
		return
	}
	keyboardHook.mu.Lock()
	keyboardHook.callback = nil
	keyboardHook.mu.Unlock()
	unhookWindowsHookEx.Call(hook)
}

func keyboardHookCallback(nCode, wParam, lParam uintptr) uintptr {
	if int32(nCode) >= 0 && lParam != 0 {
		var info keyboardHookInfo
		// Copy the native structure instead of casting uintptr to
		// unsafe.Pointer; the latter is a vet error and the copy keeps the
		// callback's ABI unchanged.
		moveMemory.Call(uintptr(unsafe.Pointer(&info)), lParam, unsafe.Sizeof(info))
		var down bool
		switch uint32(wParam) {
		case wmKeyDown, wmSysKeyDown:
			down = true
		case wmKeyUp, wmSysKeyUp:
		default:
			result, _, _ := callNextHookEx.Call(0, nCode, wParam, lParam)
			return result
		}
		keyboardHook.mu.Lock()
		callback := keyboardHook.callback
		keyboardHook.mu.Unlock()
		if callback != nil {
			callback(KeyEvent{VirtualKey: info.virtualKey, Down: down, Injected: info.flags&llkhfInjected != 0})
		}
	}
	result, _, _ := callNextHookEx.Call(0, nCode, wParam, lParam)
	return result
}

func (h *MessageHost) Close() {
	h.close.Do(func() {
		h.StopKeyboard()
		if h.hwnd != 0 {
			removeClipboardFormat.Call(h.hwnd)
			postMessage.Call(h.hwnd, wmClose, 0, 0)
		}
	})
	<-h.done
}

func messageHostWndProc(hwnd, message, wParam, lParam uintptr) uintptr {
	value, ok := messageHosts.Load(hwnd)
	if !ok {
		result, _, _ := defWindowProc.Call(hwnd, message, wParam, lParam)
		return result
	}
	host := value.(*MessageHost)
	switch uint32(message) {
	case wmClose:
		destroyWindow.Call(hwnd)
		return 0
	case wmDestroy:
		postQuitMessage.Call(0)
		return 0
	case messageHostDispatchWindow:
		host.drain()
		return 0
	case wmClipboardUpdate:
		host.mu.Lock()
		callback := host.onClipboard
		host.mu.Unlock()
		if callback != nil {
			callback()
		}
		return 0
	case wmSettingChange, wmColorizationChanged:
		host.mu.Lock()
		callback := host.onTheme
		host.mu.Unlock()
		if callback != nil {
			callback(SystemDark())
		}
		return 0
	}
	result, _, _ := defWindowProc.Call(hwnd, message, wParam, lParam)
	return result
}

// COMHost owns an apartment-threaded COM thread. COM objects created by a task
// stay on this thread; Do is synchronous and must not be called from that same
// thread or from a path that blocks the MyGo UI loop.
type COMHost struct {
	ready    chan struct{}
	done     chan struct{}
	quit     chan struct{}
	tasks    chan func()
	startErr error
	close    sync.Once
}

func StartCOMHost() (*COMHost, error) {
	host := &COMHost{
		ready: make(chan struct{}),
		done:  make(chan struct{}),
		quit:  make(chan struct{}),
		tasks: make(chan func(), 32),
	}
	go host.run()
	<-host.ready
	if host.startErr != nil {
		return nil, host.startErr
	}
	return host, nil
}

func (h *COMHost) run() {
	runtime.LockOSThread()
	defer runtime.UnlockOSThread()
	defer close(h.done)
	hr, _, _ := coInitializeEx.Call(0, coinitApartmentThread)
	if code := int32(hr); code < 0 {
		if code == rpcChangedMode {
			h.startErr = errors.New("COM thread already initialized with a different apartment")
		} else {
			h.startErr = fmt.Errorf("initialize COM apartment: 0x%08x", uint32(code))
		}
		close(h.ready)
		return
	}
	defer coUninitialize.Call()
	close(h.ready)
	for {
		select {
		case task := <-h.tasks:
			task()
		case <-h.quit:
			return
		}
	}
}

func (h *COMHost) Do(task func() error) error {
	result := make(chan error, 1)
	select {
	case h.tasks <- func() { result <- task() }:
	case <-h.done:
		return errors.New("COM apartment is closed")
	}
	select {
	case err := <-result:
		return err
	case <-h.done:
		return errors.New("COM apartment is closed")
	}
}

func (h *COMHost) Close() {
	h.close.Do(func() { close(h.quit) })
	<-h.done
}
