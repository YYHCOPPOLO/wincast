package extensions

import (
	"bytes"
	"compress/flate"
	"compress/gzip"
	"compress/zlib"
	"context"
	"crypto/hmac"
	"crypto/md5"
	"crypto/rand"
	"crypto/sha1"
	"crypto/sha256"
	"crypto/sha512"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"hash"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/YYHCOPPOLO/wincast/go/internal/domain/extension"
)

// performSync answers the Node shims that must not hop to the main actor:
// fs, child_process, crypto and zlib. It runs on the JavaScript goroutine.
func (m *Manager) performSync(api, method string, arguments []extension.Value) string {
	value, err := dispatchSync(api, method, arguments)
	if err != nil {
		var shim *shimError
		if errors.As(err, &shim) {
			return syncEnvelope(false, nil, shim.code, shim.Error())
		}
		return syncEnvelope(false, nil, "EUNKNOWN", err.Error())
	}
	return syncEnvelope(true, value, "", "")
}

type shimError struct {
	code    string
	message string
}

func (e *shimError) Error() string { return e.message }

func shimFail(code, format string, args ...any) error {
	return &shimError{code: code, message: fmt.Sprintf(format, args...)}
}

func syncEnvelope(ok bool, value any, code, message string) string {
	payload := map[string]any{"ok": ok}
	if ok {
		payload["value"] = value
	} else {
		payload["error"] = message
		if code != "" {
			payload["code"] = code
		}
	}
	data, err := json.Marshal(payload)
	if err != nil {
		return `{"ok":false,"error":"could not encode host result","code":"EUNKNOWN"}`
	}
	return string(data)
}

func dispatchSync(api, method string, arguments []extension.Value) (any, error) {
	switch api {
	case "fs":
		return syncFS(method, arguments)
	case "proc":
		return syncProcess(method, arguments)
	case "crypto":
		return syncCrypto(method, arguments)
	case "zlib":
		return syncZlib(method, arguments)
	default:
		return nil, shimFail("ENOSYS", "Unknown host module %q.", api)
	}
}

func expanded(value string) string {
	if value == "~" {
		home, _ := os.UserHomeDir()
		return home
	}
	if strings.HasPrefix(value, "~/") || strings.HasPrefix(value, `~\`) {
		home, _ := os.UserHomeDir()
		return filepath.Join(home, value[2:])
	}
	return value
}

func syncPath(arguments []extension.Value, index int, method string) (string, error) {
	if index >= len(arguments) {
		return "", shimFail("EINVAL", "fs.%s needs a path.", method)
	}
	value, ok := arguments[index].StringValue()
	if !ok || value == "" {
		return "", shimFail("EINVAL", "fs.%s needs a path.", method)
	}
	return expanded(value), nil
}

func syncFS(method string, arguments []extension.Value) (any, error) {
	switch method {
	case "readFile":
		target, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		data, err := os.ReadFile(target)
		if err != nil {
			return nil, shimFail("ENOENT", "ENOENT: no such file or directory, open '%s'", target)
		}
		return base64.StdEncoding.EncodeToString(data), nil
	case "writeFile":
		target, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		data := []byte{}
		if len(arguments) > 1 {
			if text, ok := arguments[1].StringValue(); ok {
				decoded, decodeErr := base64.StdEncoding.DecodeString(text)
				if decodeErr == nil {
					data = decoded
				}
			}
		}
		appendMode := false
		if len(arguments) > 2 {
			appendMode, _ = arguments[2].BoolValue()
		}
		flags := os.O_WRONLY | os.O_CREATE | os.O_TRUNC
		if appendMode {
			flags = os.O_WRONLY | os.O_CREATE | os.O_APPEND
		}
		file, err := os.OpenFile(target, flags, 0o600)
		if err != nil {
			return nil, shimFail("EACCES", "EACCES: could not write '%s'", target)
		}
		_, err = file.Write(data)
		closeErr := file.Close()
		if err != nil {
			return nil, shimFail("EACCES", "EACCES: could not write '%s'", target)
		}
		if closeErr != nil {
			return nil, shimFail("EACCES", "EACCES: could not write '%s'", target)
		}
		return nil, nil
	case "exists":
		target, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		_, statErr := os.Stat(target)
		return statErr == nil, nil
	case "stat":
		target, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		noFollow := false
		if len(arguments) > 1 {
			noFollow, _ = arguments[1].BoolValue()
		}
		var info os.FileInfo
		if noFollow {
			info, err = os.Lstat(target)
		} else {
			info, err = os.Stat(target)
		}
		if err != nil {
			return nil, shimFail("ENOENT", "ENOENT: no such file or directory, stat '%s'", target)
		}
		return statPayload(info), nil
	case "readdir":
		target, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		entries, err := os.ReadDir(target)
		if err != nil {
			return nil, shimFail("ENOENT", "ENOENT: no such file or directory, scandir '%s'", target)
		}
		result := make([]map[string]any, 0, len(entries))
		for _, entry := range entries {
			info, infoErr := entry.Info()
			isFile := false
			isDirectory := entry.IsDir()
			isSymlink := entry.Type()&os.ModeSymlink != 0
			if infoErr == nil {
				isFile = info.Mode().IsRegular()
			}
			result = append(result, map[string]any{
				"name":            entry.Name(),
				"parentPath":      target,
				"_isFile":         isFile,
				"_isDirectory":    isDirectory,
				"_isSymbolicLink": isSymlink,
			})
		}
		return result, nil
	case "mkdir":
		target, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		recursive := false
		if len(arguments) > 1 {
			recursive, _ = arguments[1].BoolValue()
		}
		if recursive {
			if err := os.MkdirAll(target, 0o700); err != nil {
				return nil, shimFail("EACCES", "EACCES: could not create '%s'", target)
			}
			return target, nil
		}
		if err := os.Mkdir(target, 0o700); err != nil {
			return nil, shimFail("EACCES", "EACCES: could not create '%s'", target)
		}
		return nil, nil
	case "remove":
		target, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		recursive := false
		force := false
		if len(arguments) > 1 {
			recursive, _ = arguments[1].BoolValue()
		}
		if len(arguments) > 2 {
			force, _ = arguments[2].BoolValue()
		}
		if _, statErr := os.Stat(target); statErr != nil {
			if force {
				return nil, nil
			}
			return nil, shimFail("ENOENT", "ENOENT: no such file or directory, unlink '%s'", target)
		}
		if recursive {
			if err := os.RemoveAll(target); err != nil {
				return nil, shimFail("EACCES", "EACCES: could not remove '%s'", target)
			}
			return nil, nil
		}
		if err := os.Remove(target); err != nil {
			return nil, shimFail("EACCES", "EACCES: could not remove '%s'", target)
		}
		return nil, nil
	case "rename":
		from, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		to, err := syncPath(arguments, 1, method)
		if err != nil {
			return nil, err
		}
		_ = os.RemoveAll(to)
		if err := os.Rename(from, to); err != nil {
			return nil, shimFail("EACCES", "EACCES: could not rename '%s'", from)
		}
		return nil, nil
	case "copyFile":
		from, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		to, err := syncPath(arguments, 1, method)
		if err != nil {
			return nil, err
		}
		_ = os.RemoveAll(to)
		if err := copyFile(from, to); err != nil {
			return nil, shimFail("EACCES", "EACCES: could not copy '%s'", from)
		}
		return nil, nil
	case "realpath":
		target, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		resolved, err := filepath.EvalSymlinks(target)
		if err != nil {
			return nil, shimFail("ENOENT", "ENOENT: no such file or directory, realpath '%s'", target)
		}
		return resolved, nil
	case "mkdtemp":
		prefix, err := syncPath(arguments, 0, method)
		if err != nil {
			return nil, err
		}
		suffix := make([]byte, 3)
		_, _ = rand.Read(suffix)
		target := prefix + hex.EncodeToString(suffix)
		if err := os.MkdirAll(target, 0o700); err != nil {
			return nil, shimFail("EACCES", "EACCES: could not create '%s'", target)
		}
		return target, nil
	default:
		return nil, shimFail("ENOSYS", "fs.%s is not supported.", method)
	}
}

func statPayload(info os.FileInfo) map[string]any {
	milliseconds := func(when time.Time) float64 {
		return float64(when.UnixNano()) / 1e6
	}
	return map[string]any{
		"size":            float64(info.Size()),
		"mode":            uint32(info.Mode().Perm()),
		"mtimeMs":         milliseconds(info.ModTime()),
		"atimeMs":         milliseconds(info.ModTime()),
		"ctimeMs":         milliseconds(info.ModTime()),
		"birthtimeMs":     milliseconds(info.ModTime()),
		"_isFile":         info.Mode().IsRegular(),
		"_isDirectory":    info.IsDir(),
		"_isSymbolicLink": info.Mode()&os.ModeSymlink != 0,
	}
}

func syncProcess(method string, arguments []extension.Value) (any, error) {
	if method != "run" {
		return nil, shimFail("ENOSYS", "child_process.%s is not supported.", method)
	}
	return runProcess(arguments, false)
}

func runProcess(arguments []extension.Value, async bool) (any, error) {
	object := argumentObject(arguments, 0)
	if object == nil {
		return nil, shimFail("EINVAL", "No command given.")
	}
	command, _ := object["command"].StringValue()
	if command == "" {
		return nil, shimFail("EINVAL", "No command given.")
	}
	useShell, _ := object["shell"].BoolValue()
	cwd, _ := object["cwd"].StringValue()
	if cwd != "" {
		cwd = expanded(cwd)
	}
	timeout, hasTimeout := object["timeout"].DoubleValue()
	inputBase64, _ := object["input"].StringValue()
	detached, _ := object["detached"].BoolValue()

	args := make([]string, 0)
	if values, ok := object["args"].ArrayValue(); ok {
		for _, value := range values {
			text, _ := value.StringValue()
			args = append(args, text)
		}
	}

	var commandLine string
	var commandArgs []string
	if useShell {
		if runtime.GOOS == "windows" {
			commandLine = "cmd.exe"
			commandArgs = []string{"/d", "/s", "/c", command}
		} else {
			commandLine = "/bin/sh"
			commandArgs = []string{"-c", command}
		}
	} else {
		commandLine = command
		commandArgs = args
		if !filepath.IsAbs(commandLine) {
			if resolved, err := exec.LookPath(commandLine); err == nil {
				commandLine = resolved
			} else {
				return nil, shimFail("ENOENT", "ENOENT: command not found: '%s'", command)
			}
		}
	}

	var cancel context.CancelFunc
	ctx := context.Background()
	if hasTimeout && timeout > 0 {
		ctx, cancel = context.WithTimeout(ctx, time.Duration(timeout)*time.Millisecond)
	} else {
		ctx, cancel = context.WithCancel(ctx)
	}
	var cmd *exec.Cmd
	if detached {
		cancel()
		cmd = exec.Command(commandLine, commandArgs...)
	} else {
		defer cancel()
		cmd = exec.CommandContext(ctx, commandLine, commandArgs...)
	}
	if cwd != "" {
		cmd.Dir = cwd
	}
	if environment, ok := object["env"].ObjectValue(); ok {
		cmd.Env = make([]string, 0, len(environment)+1)
		hasSystemRoot := false
		for key, value := range environment {
			text, _ := value.StringValue()
			cmd.Env = append(cmd.Env, key+"="+text)
			if strings.EqualFold(key, "SystemRoot") {
				hasSystemRoot = true
			}
		}
		if !hasSystemRoot {
			cmd.Env = append(cmd.Env, "SystemRoot="+os.Getenv("SystemRoot"))
		}
	} else {
		cmd.Env = os.Environ()
	}
	var stdout, stderr bytes.Buffer
	cmd.Stdout = &stdout
	cmd.Stderr = &stderr
	if inputBase64 != "" {
		if data, err := base64.StdEncoding.DecodeString(inputBase64); err == nil {
			cmd.Stdin = bytes.NewReader(data)
		}
	}
	if err := cmd.Start(); err != nil {
		return nil, shimFail("ENOENT", "Could not run '%s': %v", command, err)
	}
	if detached {
		_ = cmd.Process.Release()
		return map[string]any{
			"stdout": "", "stderr": "", "status": 0, "signal": nil,
		}, nil
	}
	waitErr := cmd.Wait()
	status := 0
	signal := any(nil)
	if waitErr != nil {
		var exitError *exec.ExitError
		if errors.As(waitErr, &exitError) {
			status = exitError.ExitCode()
		} else {
			status = -1
		}
		if ctx.Err() == context.DeadlineExceeded {
			status = -1
			signal = "SIGTERM"
		}
	}
	return map[string]any{
		"stdout": base64.StdEncoding.EncodeToString(stdout.Bytes()),
		"stderr": base64.StdEncoding.EncodeToString(stderr.Bytes()),
		"status": status,
		"signal": signal,
	}, nil
}

func syncCrypto(method string, arguments []extension.Value) (any, error) {
	switch method {
	case "uuid":
		bytes := make([]byte, 16)
		if _, err := rand.Read(bytes); err != nil {
			return nil, shimFail("EUNKNOWN", "could not generate a UUID")
		}
		bytes[6] = (bytes[6] & 0x0f) | 0x40
		bytes[8] = (bytes[8] & 0x3f) | 0x80
		return fmt.Sprintf("%x-%x-%x-%x-%x", bytes[0:4], bytes[4:6], bytes[6:8], bytes[8:10], bytes[10:16]), nil
	case "random":
		count := 0
		if len(arguments) > 0 {
			if value, ok := arguments[0].DoubleValue(); ok {
				count = int(value)
			}
		}
		if count < 0 {
			count = 0
		}
		bytes := make([]byte, count)
		if _, err := rand.Read(bytes); err != nil {
			return nil, shimFail("EUNKNOWN", "could not generate random bytes")
		}
		return base64.StdEncoding.EncodeToString(bytes), nil
	case "hash", "hmac":
		algorithm := "sha256"
		if len(arguments) > 0 {
			if value, ok := arguments[0].StringValue(); ok && value != "" {
				algorithm = strings.ToLower(value)
			}
		}
		data := []byte{}
		if len(arguments) > 1 {
			if text, ok := arguments[1].StringValue(); ok {
				decoded, _ := base64.StdEncoding.DecodeString(text)
				data = decoded
			}
		}
		if method == "hash" {
			digest := hashBytes(algorithm, data)
			if digest == nil {
				return nil, shimFail("ENOSYS", "Unsupported hash algorithm '%s'.", algorithm)
			}
			return base64.StdEncoding.EncodeToString(digest), nil
		}
		key := []byte{}
		if len(arguments) > 2 {
			if text, ok := arguments[2].StringValue(); ok {
				decoded, _ := base64.StdEncoding.DecodeString(text)
				key = decoded
			}
		}
		digest := hmacBytes(algorithm, data, key)
		if digest == nil {
			return nil, shimFail("ENOSYS", "Unsupported HMAC algorithm '%s'.", algorithm)
		}
		return base64.StdEncoding.EncodeToString(digest), nil
	}
	return nil, shimFail("ENOSYS", "crypto.%s is not supported.", method)
}

func hashBytes(algorithm string, data []byte) []byte {
	switch algorithm {
	case "md5":
		sum := md5.Sum(data)
		return sum[:]
	case "sha1":
		sum := sha1.Sum(data)
		return sum[:]
	case "sha256":
		sum := sha256.Sum256(data)
		return sum[:]
	case "sha384":
		sum := sha512.Sum384(data)
		return sum[:]
	case "sha512":
		sum := sha512.Sum512(data)
		return sum[:]
	default:
		return nil
	}
}

func hmacBytes(algorithm string, data, key []byte) []byte {
	var constructor func() hash.Hash
	switch algorithm {
	case "md5":
		constructor = md5.New
	case "sha1":
		constructor = sha1.New
	case "sha256":
		constructor = sha256.New
	case "sha384":
		constructor = sha512.New384
	case "sha512":
		constructor = sha512.New
	default:
		return nil
	}
	mac := hmac.New(constructor, key)
	mac.Write(data)
	return mac.Sum(nil)
}

func syncZlib(method string, arguments []extension.Value) (any, error) {
	data := []byte{}
	if len(arguments) > 0 {
		if text, ok := arguments[0].StringValue(); ok {
			decoded, _ := base64.StdEncoding.DecodeString(text)
			data = decoded
		}
	}
	var output []byte
	var err error
	switch method {
	case "gzip":
		var buffer bytes.Buffer
		writer := gzip.NewWriter(&buffer)
		if _, err = writer.Write(data); err == nil {
			err = writer.Close()
		}
		output = buffer.Bytes()
	case "gunzip":
		var reader *gzip.Reader
		if reader, err = gzip.NewReader(bytes.NewReader(data)); err == nil {
			output, err = io.ReadAll(reader)
			reader.Close()
		}
	case "deflate":
		var buffer bytes.Buffer
		writer := zlib.NewWriter(&buffer)
		if _, err = writer.Write(data); err == nil {
			err = writer.Close()
		}
		output = buffer.Bytes()
	case "inflate":
		var reader io.ReadCloser
		if reader, err = zlib.NewReader(bytes.NewReader(data)); err == nil {
			output, err = io.ReadAll(reader)
			reader.Close()
		}
	case "deflateRaw":
		var buffer bytes.Buffer
		writer, writerErr := flate.NewWriter(&buffer, flate.DefaultCompression)
		err = writerErr
		if writerErr == nil {
			if _, err = writer.Write(data); err == nil {
				err = writer.Close()
			}
		}
		output = buffer.Bytes()
	case "inflateRaw":
		reader := flate.NewReader(bytes.NewReader(data))
		output, err = io.ReadAll(reader)
		reader.Close()
	default:
		return nil, shimFail("ENOSYS", "zlib.%s is not supported.", method)
	}
	if err != nil {
		return nil, shimFail("EUNKNOWN", "zlib.%s failed: %v", method, err)
	}
	return base64.StdEncoding.EncodeToString(output), nil
}
