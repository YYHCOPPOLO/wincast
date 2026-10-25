# Tinycast for Windows

Windows 11 上的原生启动器。单个 exe，无 Electron / WebView / 遥测；界面与交互对齐 macOS 版 [Tinycast](https://github.com/abue-ammar/tinycast)，用 Rust + Slint 独立重写。

**简体中文** | [English](README.en.md)

从 [Releases](https://github.com/YYHCOPPOLO/wincast/releases/latest) 下载。

## 功能

- **应用启动器**：模糊搜索开始菜单、AppX 和自定义目录；收藏槽 Ctrl+1…9 / Ctrl+0
- **系统操作**：锁屏、睡眠、音量、深色模式、回收站、`ms-settings:` 页面
- **剪贴板历史**：文本与图片，支持搜索、钉住、粘回唤出前的窗口
- **计算器**：数学、单位、汇率、日期；Enter 复制并写入历史
- **词典**：输入英文单词或短语后按 Tab，显示 ECDICT 的全部释义（本地数据，不调用 AI）
- **命令与快捷链接**：自定义命令、链接和全局热键
- **窗口管理**：贴边、三分、虚拟桌面
- **文件搜索**：Windows Search，或有限深度的目录遍历
- **片段**：Markdown 模板、占位符、关键字展开
- **Notes / 日历 / Emoji**：Markdown 笔记、会议入会卡片、Emoji 选择
- **AI Chat / Quick Actions**：OpenAI、Anthropic、Gemini、OpenRouter 或兼容端点
- **备份**：设置导出/导入，可尝试导入 Raycast `.rayconfig`

以下功能默认关闭，在“设置”中开启：文件搜索、Notes、片段、窗口管理、日历、AI、Quick Actions、快捷链接、自定义命令、扩展。

## 安装

1. 从 [Releases](https://github.com/YYHCOPPOLO/wincast/releases/latest) 下载 `tinycast-*-windows-x64.zip`，解压到任意目录。
2. 运行 `tinycast.exe`，程序常驻托盘。
3. 按 **Alt+Space** 唤出。

需要 **Windows 11 24H2（build 26100）及以上**，x64。

## 使用

| 操作 | 按键 |
| --- | --- |
| 显示 / 隐藏面板 | Alt+Space（可在设置中修改） |
| 启动当前结果 | Enter |
| 清空 / 关闭 | Esc |
| 启动器 / 查词 / AI / 剪贴板 | Tab |
| 当前行操作 | Ctrl+K |
| 收藏槽 | Ctrl+1…9、Ctrl+0 |

设置：右键托盘图标 → 设置。语言在“通用”里切换 **简体中文 / English**，立即生效。

## 从源码构建

需要 Rust stable（MSVC）与 Visual Studio 2022 Build Tools（MSVC 和 Windows SDK）。

```powershell
cargo build -p tinycast --release
.\target\release\tinycast.exe
```

词典数据不在 exe 里：先从 [ECDICT](https://github.com/skywind3000/ECDICT)（固定提交）下载 CSV，再本地构建一次（约 97 MB，770,611 条）：

```powershell
pwsh -NoProfile -File tools/build-dict.ps1
```

脚本把 CSV 缓存到 `%TEMP%`，生成 `%APPDATA%\com.tinycast.win\Dictionary\ecdict.sqlite`，并在旁边写入 MIT 许可。`-Force` 可重建；没有数据库时 Tab 仍是原来的 启动器 → AI/剪贴板。

## 数据

| 内容 | 位置 |
| --- | --- |
| 设置、聊天、笔记、片段、学习排序 | `%APPDATA%\com.tinycast.win\` |
| 剪贴板历史与图片缓存 | exe 同目录下的 `data\` |
| 词典数据（ECDICT，由 `tools/build-dict.ps1` 生成） | `%APPDATA%\com.tinycast.win\Dictionary\` |
| API Key | DPAPI 加密，不进入备份 |

## 许可

MIT。界面与产品行为参考 [Tinycast](https://github.com/abue-ammar/tinycast)（AGPL-3.0）；本仓库为独立重写，不包含其 Swift 源码。

词典数据来自 [ECDICT](https://github.com/skywind3000/ECDICT)（MIT，Copyright (c) 2025 Linwei），固定提交 `9c46923f`；构建脚本会把许可文本写入数据库同目录的 `LICENSE-ECDICT.txt`。
