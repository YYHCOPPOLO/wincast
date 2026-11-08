# wincast

Windows 11 上的原生启动器。单个 exe，无 Electron / WebView / 遥测；界面与交互对齐 macOS 版 [Tinycast](https://github.com/abue-ammar/tinycast)，用 Go + MyGo 原生 UI 独立重写。

**简体中文** | [English](README.en.md)

从 [Releases](https://github.com/YYHCOPPOLO/wincast/releases/latest) 下载安装版或便携版。项目现在仅使用 Go + MyGo，主分支为 `main`。

## 功能

- **应用启动器**：模糊搜索开始菜单、AppX 和自定义目录；收藏槽 Ctrl+1…9 / Ctrl+0
- **系统操作**：锁屏、睡眠、音量、深色模式、回收站、`ms-settings:` 页面
- **剪贴板历史**：Ctrl+~ 呼出；文本与图片，支持搜索、钉住、粘回唤出前的窗口
- **计算器**：数学、单位、汇率、日期；Enter 复制并写入历史
- **词典**：输入英文单词或短语后按 Tab，显示 ECDICT 的全部释义（本地数据，不调用 AI）
- **命令与快捷链接**：自定义命令、链接和全局热键
- **窗口管理**：贴边、三分、虚拟桌面
- **文件搜索**：Windows Search，或有限深度的目录遍历
- **片段**：Markdown 模板、占位符、关键字展开
- **Notes / 日历 / Emoji**：Markdown 笔记、会议入会卡片、Emoji 选择
- **AI Chat / Quick Actions**：OpenAI、Anthropic、Gemini、OpenRouter 或兼容端点
- **备份**：设置导出/导入，可尝试导入 Raycast `.rayconfig`

以下功能默认关闭，在“设置”中开启：文件搜索、Notes、片段、窗口管理、日历、AI、Quick Actions、快捷链接、自定义命令。

第三方 JavaScript 扩展、AI 订阅登录及清除系统通知目前不可用。

## 安装

1. 构建或取得 Go 产物后，复制到任意目录；路径可以包含空格和中文。
2. 运行 `wincast.exe`，程序常驻托盘。
3. 按 **Alt+Space** 唤出。

首次安装可运行安装包，也可直接使用便携版。

需要 **Windows 11 24H2（build 26100）及以上**，x64。

## 使用

| 操作 | 按键 |
| --- | --- |
| 显示 / 隐藏面板 | Alt+Space（可在设置中修改） |
| 启动当前结果 | Enter |
| 清空 / 关闭 | Esc |
| 启动器 / 查词 / AI | Tab |
| 剪贴板历史 | Ctrl+~ |
| 当前行操作 | Ctrl+K |
| 收藏槽 | Ctrl+1…9、Ctrl+0 |

设置：右键托盘图标 → 设置。语言在“通用”里切换 **简体中文 / English**，立即生效。

## 从源码构建

需要 [Go 1.27.3](https://go.dev/dl/)；MyGo v0.6.4 作为固定版本的 Go tool 依赖，由 `go.mod` 拉取，不需要单独安装。

```powershell
cd go
go tool mygo build -platform windows/amd64
.\build\windows-amd64\wincast.exe
```

生产产物同时生成 NSIS 安装包；`tools/package-release.ps1` 会把 exe、图标、第三方许可和 `release.json`（含 SHA-256）整理到 `go\build\release\`，不会上传或签名。

词典数据不在 exe 里：先从 [ECDICT](https://github.com/skywind3000/ECDICT)（固定提交）下载 CSV，再本地构建一次（约 125 MB，770,611 条）：

```powershell
pwsh -NoProfile -File tools/build-dict.ps1
```

脚本把 CSV 缓存到 `%TEMP%`，生成 `%LOCALAPPDATA%\com.wincast.win\Dictionary\ecdict.sqlite`，并在旁边写入 MIT 许可。`-Force` 可重建；没有数据库时 Tab 只在 启动器 ↔ AI 之间切换，剪贴板历史始终用 Ctrl+~ 打开。

## 数据

首次启动会把旧 Go 版 `%APPDATA%\com.tinycast.win.go-preview\` 复制到新目录，不删除旧数据。便携版升级时保留 exe 所在目录及 `data` 文件夹。备份导出为 `.wincast`，仍可导入旧 `.tinycast` 文件。

| 内容 | 位置 |
| --- | --- |
| 设置、聊天、笔记、片段、学习排序（Go 版） | `%APPDATA%\com.wincast.win\` |
| 剪贴板历史与图片缓存（Go 版） | exe 同目录下的 `data\` |
| 历史数据（保留，用于导入） | `%APPDATA%\com.tinycast.win\` |
| 词典数据（ECDICT，由 `tools/build-dict.ps1` 生成） | `%LOCALAPPDATA%\com.wincast.win\Dictionary\`（仍可读取旧目录中的现有词典） |
| 日志 | `%LOCALAPPDATA%\com.wincast.win\logs\wincast.log` |
| API Key | DPAPI 加密，不进入备份 |

## 许可

MIT。Wincast 的界面与产品行为参考 [Tinycast](https://github.com/abue-ammar/tinycast)（AGPL-3.0）；本仓库为独立重写，不包含其 Swift 源码。

词典数据来自 [ECDICT](https://github.com/skywind3000/ECDICT)（MIT，Copyright (c) 2025 Linwei），固定提交 `9c46923f`；构建脚本会把许可文本写入数据库同目录的 `LICENSE-ECDICT.txt`。
