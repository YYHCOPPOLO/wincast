# wincast

A native launcher for Windows 11. One exe, no Electron, no WebView, no telemetry. The UI and behavior follow the macOS [Tinycast](https://github.com/abue-ammar/tinycast); this is an independent Go + MyGo native-UI rewrite.

[简体中文](README.md) | **English**

Download the installer or portable build from [Releases](https://github.com/YYHCOPPOLO/wincast/releases/latest). The project uses Go + MyGo on `main`.

## Features

- **App launcher**: fuzzy search Start Menu, AppX and custom folders; favorite slots Ctrl+1…9 / Ctrl+0
- **System actions**: lock, sleep, volume, dark mode, recycle bin, `ms-settings:` pages
- **Clipboard history**: opens with Ctrl+~; text and images, search, pin, paste back to the previous window
- **Calculator**: math, units, currencies, dates; Enter copies and saves history
- **Dictionary**: type an English word or phrase and press Tab for the whole ECDICT entry (local data, no AI call)
- **Commands & quicklinks**: custom commands, links and global hotkeys
- **Window management**: snap, thirds, virtual desktops
- **File search**: Windows Search, or a bounded folder walk
- **Snippets**: Markdown templates, placeholders, keyword expansion
- **Notes / Calendar / Emoji**: Markdown notes, meeting join cards, emoji picker
- **AI Chat / Quick Actions**: OpenAI, Anthropic, Gemini, OpenRouter or compatible endpoints
- **Backup**: settings export/import, plus Raycast `.rayconfig` import

Off by default (enable in Settings): file search, Notes, snippets, window management, calendar, AI, Quick Actions, quicklinks, custom commands.

Third-party JavaScript extensions, AI subscription login and dismissing system notifications are currently unavailable.

## Install

1. Build the Go artifacts or copy them anywhere; the path may contain spaces or non-ASCII characters.
2. Run `wincast.exe`. It stays in the tray.
3. Press **Alt+Space**.

Requires **Windows 11 24H2 (build 26100) or later**, x64.

## Usage

| Action | Keys |
| --- | --- |
| Show / hide the palette | Alt+Space (configurable) |
| Run the selected result | Enter |
| Clear / close | Esc |
| Launcher / dictionary / AI | Tab |
| Clipboard history | Ctrl+~ |
| Actions for the current row | Ctrl+K |
| Favorite slots | Ctrl+1…9, Ctrl+0 |

Settings: right-click the tray icon → Settings. Switch **简体中文 / English** under General; it applies immediately.

## Build from source

Requires [Go 1.27.3](https://go.dev/dl/); MyGo v0.6.4 is a pinned Go tool dependency from `go.mod`, not a separate install.

```powershell
cd go
go tool mygo build -platform windows/amd64
.\build\windows-amd64\wincast.exe
```

The production build also writes an NSIS installer. `tools/package-release.ps1` stages the exe, icon, third-party licenses and a `release.json` with SHA-256 hashes under `go\build\release\`; it does not upload or sign anything.

The dictionary data stays out of the exe: build it once from [ECDICT](https://github.com/skywind3000/ECDICT) at the pinned commit (~125 MB, 770,611 entries):

```powershell
pwsh -NoProfile -File tools/build-dict.ps1
```

The script caches the CSV in `%TEMP%`, writes `%LOCALAPPDATA%\com.wincast.win\Dictionary\ecdict.sqlite`, and drops the MIT notice next to it. `-Force` rebuilds; without the database Tab only rings between the launcher and AI, and clipboard history always opens with Ctrl+~.

## Data

On first launch, the previous Go profile at `%APPDATA%\com.tinycast.win.go-preview\` is copied to the new profile without deleting the original. For portable upgrades keep the executable in its original directory and preserve `data`. Backups use `.wincast`; older `.tinycast` backups remain importable.

| What | Where |
| --- | --- |
| Settings, chats, notes, snippets, learning rank (Go build) | `%APPDATA%\com.wincast.win\` |
| Clipboard history and image cache (Go build) | `data\` next to the exe |
| Previous data (preserved for import) | `%APPDATA%\com.tinycast.win\` |
| Dictionary data (ECDICT, built by `tools/build-dict.ps1`) | `%LOCALAPPDATA%\com.wincast.win\Dictionary\` (falls back to the previous dictionary directory) |
| Logs | `%LOCALAPPDATA%\com.wincast.win\logs\wincast.log` |
| API keys | DPAPI-protected, excluded from backups |

## License

MIT. Wincast is an independent rewrite; the UI and behavior reference [Tinycast](https://github.com/abue-ammar/tinycast) (AGPL-3.0) without copying its Swift source.

Dictionary data comes from [ECDICT](https://github.com/skywind3000/ECDICT) (MIT, Copyright (c) 2025 Linwei) at commit `9c46923f`; the build script writes that license to `LICENSE-ECDICT.txt` next to the database.
