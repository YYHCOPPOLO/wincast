# Tinycast for Windows

A native launcher for Windows 11. One exe, no Electron, no WebView, no telemetry. The UI and behavior follow the macOS [Tinycast](https://github.com/abue-ammar/tinycast); this is an independent Rust + Slint rewrite.

[简体中文](README.md) | **English**

Download from [Releases](https://github.com/YYHCOPPOLO/wincast/releases/latest).

## Features

- **App launcher**: fuzzy search Start Menu, AppX and custom folders; favorite slots Ctrl+1…9 / Ctrl+0
- **System actions**: lock, sleep, volume, dark mode, recycle bin, `ms-settings:` pages
- **Clipboard history**: text and images, search, pin, paste back to the previous window
- **Calculator**: math, units, currencies, dates; Enter copies and saves history
- **Commands & quicklinks**: custom commands, links and global hotkeys
- **Window management**: snap, thirds, virtual desktops
- **File search**: Windows Search, or a bounded folder walk
- **Snippets**: Markdown templates, placeholders, keyword expansion
- **Notes / Calendar / Emoji**: Markdown notes, meeting join cards, emoji picker
- **AI Chat / Quick Actions**: OpenAI, Anthropic, Gemini, OpenRouter or compatible endpoints
- **Backup**: settings export/import, plus Raycast `.rayconfig` import

Off by default (enable in Settings): file search, Notes, snippets, window management, calendar, AI, Quick Actions, quicklinks, custom commands, extensions.

## Install

1. Download `tinycast-*-windows-x64.zip` from [Releases](https://github.com/YYHCOPPOLO/wincast/releases/latest) and unpack it anywhere.
2. Run `tinycast.exe`. It stays in the tray.
3. Press **Alt+Space**.

Requires **Windows 11 24H2 (build 26100) or later**, x64.

## Usage

| Action | Keys |
| --- | --- |
| Show / hide the palette | Alt+Space (configurable) |
| Run the selected result | Enter |
| Clear / close | Esc |
| Launcher / AI / Clipboard | Tab |
| Actions for the current row | Ctrl+K |
| Favorite slots | Ctrl+1…9, Ctrl+0 |

Settings: right-click the tray icon → Settings. Switch **简体中文 / English** under General; it applies immediately.

## Build from source

Rust stable (MSVC) plus Visual Studio 2022 Build Tools (MSVC and Windows SDK).

```powershell
cargo build -p tinycast --release
.\target\release\tinycast.exe
```

## Data

| What | Where |
| --- | --- |
| Settings, chats, notes, snippets, learning rank | `%APPDATA%\com.tinycast.win\` |
| Clipboard history and image cache | `data\` next to the exe |
| API keys | DPAPI-protected, excluded from backups |

## License

MIT. An independent rewrite; the UI and behavior reference [Tinycast](https://github.com/abue-ammar/tinycast) (AGPL-3.0) without copying its Swift source.
