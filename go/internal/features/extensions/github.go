package extensions

import (
	"bytes"
	"encoding/json"
	"fmt"
	"net/url"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// githubEntry is one entry of GitHub's contents API.
type githubEntry struct {
	Name        string `json:"name"`
	Path        string `json:"path"`
	Type        string `json:"type"`
	DownloadURL string `json:"download_url"`
}

func (m *Manager) prepareGitHub(source ListingSource, workspace string) (string, error) {
	if source.Owner == "" || source.Repository == "" || source.Path == "" {
		return "", fmt.Errorf("The GitHub registry entry is incomplete.")
	}
	root := filepath.Join(workspace, "source")
	if err := downloadGitHubFolder(source.Owner, source.Repository, source.Path, source.Ref, root); err != nil {
		return "", err
	}
	return m.buildSource(root, filepath.Join(workspace, "build"))
}

func downloadGitHubFolder(owner, repository, path, ref, destination string) error {
	if ref == "" {
		ref = "main"
	}
	api := fmt.Sprintf("https://api.github.com/repos/%s/%s/contents/%s?ref=%s",
		url.PathEscape(owner), url.PathEscape(repository), escapePath(path), url.QueryEscape(ref))
	data, err := httpGet(api)
	if err != nil {
		return err
	}
	var entries []githubEntry
	if err := json.Unmarshal(data, &entries); err != nil {
		return fmt.Errorf("read the GitHub folder: %w", err)
	}
	if err := os.MkdirAll(destination, 0o700); err != nil {
		return err
	}
	for _, entry := range entries {
		switch entry.Type {
		case "dir":
			if entry.Name == "node_modules" || entry.Name == "metadata" || entry.Name == ".git" {
				continue
			}
			if err := downloadGitHubFolder(owner, repository, entry.Path, ref,
				filepath.Join(destination, entry.Name)); err != nil {
				return err
			}
		case "file":
			if entry.DownloadURL == "" {
				continue
			}
			body, err := httpGet(entry.DownloadURL)
			if err != nil {
				return err
			}
			if err := os.WriteFile(filepath.Join(destination, entry.Name), body, 0o600); err != nil {
				return err
			}
		}
	}
	return nil
}

func escapePath(path string) string {
	parts := strings.Split(path, "/")
	for index, part := range parts {
		parts[index] = url.PathEscape(part)
	}
	return strings.Join(parts, "/")
}

type packageManager struct {
	executable string
	install    []string
	build      []string
}

func (m *Manager) resolvePackageManager() (packageManager, error) {
	candidates := []packageManager{
		{executable: "pnpm", install: []string{"install", "--ignore-scripts"}, build: []string{"run", "build"}},
		{executable: "bun", install: []string{"install", "--ignore-scripts"}, build: []string{"run", "build"}},
		{executable: "yarn", install: []string{"install", "--ignore-scripts"}, build: []string{"run", "build"}},
		{executable: "npm", install: []string{"install", "--ignore-scripts"}, build: []string{"run", "build"}},
	}
	for _, candidate := range candidates {
		if path, err := exec.LookPath(candidate.executable); err == nil {
			candidate.executable = path
			return candidate, nil
		}
		for _, directory := range strings.Split(m.searchPath(), ";") {
			if directory == "" {
				continue
			}
			for _, name := range []string{candidate.executable + ".cmd", candidate.executable + ".exe", candidate.executable} {
				path := filepath.Join(directory, name)
				if info, err := os.Stat(path); err == nil && !info.IsDir() {
					candidate.executable = path
					return candidate, nil
				}
			}
		}
	}
	return packageManager{}, fmt.Errorf("No package manager was found. Install pnpm, Bun, Yarn or npm to build extensions from source.")
}

// buildSource runs the extension's own toolchain. When ray is installed the
// build goes to a sibling output directory; otherwise the package's build
// script is used and the source itself is installed.
func (m *Manager) buildSource(source, output string) (string, error) {
	manager, err := m.resolvePackageManager()
	if err != nil {
		return "", err
	}
	node, err := exec.LookPath("node")
	if err != nil {
		node = ""
		for _, directory := range strings.Split(m.searchPath(), ";") {
			if directory == "" {
				continue
			}
			for _, name := range []string{"node.exe", "node"} {
				path := filepath.Join(directory, name)
				if info, err := os.Stat(path); err == nil && !info.IsDir() {
					node = path
					break
				}
			}
			if node != "" {
				break
			}
		}
		if node == "" {
			return "", fmt.Errorf("Node.js was not found. Install Node 22 or newer to build extensions from source.")
		}
	}
	environment := append(os.Environ(), "CI=1", "NO_COLOR=1")
	environment = append(environment, "PATH="+filepath.Dir(node)+string(os.PathListSeparator)+os.Getenv("PATH"))
	if err := runTool(manager.executable, manager.install, source, environment); err != nil {
		return "", err
	}
	ray := filepath.Join(source, "node_modules", ".bin", "ray.cmd")
	if _, err := os.Stat(ray); err != nil {
		ray = filepath.Join(source, "node_modules", ".bin", "ray")
	}
	if _, err := os.Stat(ray); err == nil {
		if err := os.MkdirAll(output, 0o700); err != nil {
			return "", err
		}
		if err := runTool(ray, []string{"build", "-e", "dist", "-o", output, "--non-interactive"}, source, environment); err != nil {
			return "", err
		}
		if _, err := os.Stat(filepath.Join(output, "package.json")); err != nil {
			return "", fmt.Errorf("The extension build produced no package.json.")
		}
		return output, nil
	}
	if err := runTool(manager.executable, manager.build, source, environment); err != nil {
		return "", err
	}
	if _, err := os.Stat(filepath.Join(source, "package.json")); err != nil {
		return "", fmt.Errorf("The extension build produced no package.json.")
	}
	return source, nil
}

func runTool(executable string, arguments []string, directory string, environment []string) error {
	var command *exec.Cmd
	if strings.HasSuffix(strings.ToLower(executable), ".cmd") || strings.HasSuffix(strings.ToLower(executable), ".bat") {
		command = exec.Command("cmd.exe", append([]string{"/d", "/s", "/c", executable}, arguments...)...)
	} else {
		command = exec.Command(executable, arguments...)
	}
	command.Dir = directory
	command.Env = environment
	var output bytes.Buffer
	command.Stdout = &output
	command.Stderr = &output
	done := make(chan error, 1)
	if err := command.Start(); err != nil {
		return err
	}
	go func() { done <- command.Wait() }()
	select {
	case err := <-done:
		if err != nil {
			return fmt.Errorf("%s failed: %s", filepath.Base(executable), tail(output.String()))
		}
		return nil
	case <-time.After(5 * time.Minute):
		_ = command.Process.Kill()
		return fmt.Errorf("%s timed out.", filepath.Base(executable))
	}
}

func tail(text string) string {
	lines := strings.Split(strings.TrimSpace(text), "\n")
	if len(lines) > 8 {
		lines = lines[len(lines)-8:]
	}
	joined := strings.TrimSpace(strings.Join(lines, "\n"))
	if joined == "" {
		return "no output"
	}
	return joined
}
