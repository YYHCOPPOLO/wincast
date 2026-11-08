// Package uninstall discovers an installed program and the files it left
// behind, then asks the pure rules which items may be offered. Nothing here
// deletes: the platform layer only moves selected paths to the Recycle Bin.
package uninstall

import (
	"context"
	"errors"
	"os"
	"path/filepath"
	"strings"

	domain "github.com/YYHCOPPOLO/wincast/go/internal/domain/uninstall"
	"github.com/YYHCOPPOLO/wincast/go/internal/platform/windows"
)

var ErrSelf = errors.New("Wincast cannot uninstall itself")

type Snapshot struct {
	Target     string
	Install    string
	Candidates []domain.Candidate
}

type Scanner struct{}

func New() *Scanner { return &Scanner{} }

// Scan resolves the launcher entry to an Add/Remove Programs entry, collects
// install and per-user folders, and returns only paths that pass protection.
func (s *Scanner) Scan(ctx context.Context, appID, name, appPath string) (Snapshot, error) {
	if isSelf(appID, name, appPath) {
		return Snapshot{}, ErrSelf
	}
	roots := machineRoots()
	programs := windows.InstalledPrograms()
	program, found := matchProgram(programs, name, appPath)
	target := name
	if found && program.DisplayName != "" {
		target = program.DisplayName
	}
	others := make([]string, 0, len(programs)*2)
	for _, candidate := range programs {
		if found && candidate.Hive == program.Hive && candidate.Key == program.Key {
			continue
		}
		if ignitable(candidate) {
			continue
		}
		others = append(others, candidate.DisplayName)
		if base := programBaseName(candidate.InstallLocation); base != "" {
			others = append(others, base)
		}
		if base := programBaseName(candidate.DisplayIcon); base != "" {
			others = append(others, base)
		}
	}

	var paths []string
	var kinds []domain.Kind
	add := func(path string, kind domain.Kind) {
		if cleaned, ok := domain.NormalizePath(path); ok {
			for _, existing := range paths {
				if domain.Fold(existing) == domain.Fold(cleaned) {
					return
				}
			}
			paths = append(paths, cleaned)
			kinds = append(kinds, kind)
		}
	}
	if found {
		add(program.InstallLocation, domain.KindInstall)
		if program.DisplayIcon != "" {
			add(filepath.Dir(program.DisplayIcon), domain.KindInstall)
		}
	}
	if info, err := os.Stat(appPath); err == nil {
		if info.IsDir() {
			add(appPath, domain.KindInstall)
		} else {
			add(filepath.Dir(appPath), domain.KindInstall)
		}
	}
	install := ""
	if len(paths) > 0 {
		install = paths[0]
	}

	safeName := domain.SafeName(target, others)
	if safeName {
		add(profilePath(roots.AppData, target), domain.KindAppData)
		add(profilePath(roots.LocalAppData, target), domain.KindLocalAppData)
		add(profilePath(roots.LocalAppData, "Programs", target), domain.KindLocalAppData)
		add(profilePath(roots.ProgramData, target), domain.KindProgramData)
	}
	for _, name := range alternateNames(target, appPath, program, found, others) {
		add(profilePath(roots.AppData, name), domain.KindAppData)
		add(profilePath(roots.LocalAppData, name), domain.KindLocalAppData)
		add(profilePath(roots.LocalAppData, "Programs", name), domain.KindLocalAppData)
		add(profilePath(roots.ProgramData, name), domain.KindProgramData)
	}

	candidates := make([]domain.Candidate, 0, len(paths))
	for index, path := range paths {
		if err := ctx.Err(); err != nil {
			return Snapshot{}, err
		}
		info, err := os.Lstat(path)
		if err != nil {
			continue
		}
		locked, reason := windows.PathLocked(path)
		facts := domain.Facts{Exists: true, IsDir: info.IsDir(), Locked: locked, Reason: reason}
		candidate, ok := domain.Classify(target, domain.Abbreviate(path, roots), path, kinds[index], facts, roots)
		if ok {
			candidates = append(candidates, candidate)
		}
	}
	candidates = domain.DedupeNested(candidates)
	return Snapshot{Target: target, Install: install, Candidates: candidates}, nil
}

// alternateNames are product folder names an app uses in its per-user data:
// the executable base name and the install-folder name. Each still has to
// pass the shared-name gate.
func alternateNames(target, appPath string, program windows.InstalledProgram, found bool, others []string) []string {
	var names []string
	add := func(name string) {
		name = strings.TrimSpace(name)
		if name == "" || strings.EqualFold(name, target) || !domain.SafeName(name, others) {
			return
		}
		for _, existing := range names {
			if strings.EqualFold(existing, name) {
				return
			}
		}
		names = append(names, name)
	}
	paths := []string{appPath}
	if found {
		paths = append(paths, program.DisplayIcon, program.InstallLocation)
	}
	for _, path := range paths {
		add(programBaseName(path))
	}
	return names
}

func profilePath(root string, parts ...string) string {
	if strings.TrimSpace(root) == "" {
		return ""
	}
	return filepath.Join(append([]string{root}, parts...)...)
}

func programBaseName(path string) string {
	path = strings.TrimSpace(path)
	if path == "" {
		return ""
	}
	base := filepath.Base(strings.TrimRight(path, `\/`))
	if strings.EqualFold(filepath.Ext(base), ".exe") {
		base = strings.TrimSuffix(base, filepath.Ext(base))
	}
	return base
}

func isSelf(appID, name, appPath string) bool {
	if strings.Contains(domain.Fold(appID), "com.wincast.win") || strings.Contains(domain.Fold(name), "wincast") {
		return true
	}
	exe, err := os.Executable()
	if err != nil {
		return false
	}
	ownRoot := filepath.Dir(exe)
	if strings.EqualFold(filepath.Clean(appPath), filepath.Clean(exe)) {
		return true
	}
	if appPath != "" {
		if absolute, err := filepath.Abs(appPath); err == nil {
			if strings.EqualFold(filepath.Dir(absolute), ownRoot) {
				return true
			}
		}
	}
	return false
}

func ignitable(program windows.InstalledProgram) bool {
	if program.SystemComponent || program.ParentKeyName != "" {
		return true
	}
	release := domain.Fold(program.ReleaseType)
	if strings.Contains(release, "update") || strings.Contains(release, "hotfix") {
		return true
	}
	name := domain.Fold(program.DisplayName)
	return strings.HasPrefix(name, "update for ") || strings.HasPrefix(name, "security update for ") ||
		strings.HasPrefix(name, "hotfix for ")
}

func matchProgram(programs []windows.InstalledProgram, name, appPath string) (windows.InstalledProgram, bool) {
	foldedName := domain.Fold(name)
	appPath = domain.Fold(appPath)
	best := windows.InstalledProgram{}
	bestScore := 0
	for _, program := range programs {
		if ignitable(program) {
			continue
		}
		score := 0
		if domain.Fold(program.DisplayName) == foldedName {
			score += 100
		}
		if appPath != "" {
			appDir := domain.Fold(filepath.Dir(appPath))
			for _, candidate := range []string{program.InstallLocation, program.DisplayIcon} {
				if candidate == "" {
					continue
				}
				folded := domain.Fold(candidate)
				if folded == appPath || folded == appDir {
					score += 300
				} else if domain.IsDescendant(appPath, folded) || domain.IsDescendant(appDir, folded) {
					score += 200
				}
			}
		}
		if score > bestScore {
			best, bestScore = program, score
		}
	}
	return best, bestScore > 0
}

func machineRoots() domain.Roots {
	exe, _ := os.Executable()
	ownRoot := ""
	if exe != "" {
		ownRoot = filepath.Dir(exe)
	}
	return domain.Roots{
		Home:         os.Getenv("USERPROFILE"),
		OwnExe:       exe,
		OwnRoot:      ownRoot,
		Windows:      os.Getenv("SystemRoot"),
		ProgramFiles: []string{os.Getenv("ProgramFiles"), os.Getenv("ProgramFiles(x86)"), os.Getenv("ProgramW6432")},
		ProgramData:  os.Getenv("ProgramData"),
		AppData:      os.Getenv("APPDATA"),
		LocalAppData: os.Getenv("LOCALAPPDATA"),
	}
}
