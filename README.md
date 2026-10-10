# Wincast

Windows 上的原生启动器。行为与界面对齐 macOS 上的 [Tinycast](https://github.com/abue-ammar/tinycast) 上游 `main`（当前对照提交 [`da2c4e44`](https://github.com/abue-ammar/tinycast/commit/da2c4e44)；此前的对照基准是 v0.10.2），用 Rust + Win32 + Direct2D 独立重写，不复制 Swift 源码。界面 token 以上游 `Tinycast/DesignSystem/Theme.swift` 与 `docs/ui.md` 为准，见 `crates/tinycast-pure/src/theme.rs`。

A tiny native Windows launcher — one exe, no Electron, no WebView, no telemetry.

Release 体积约 **2.6 MB**（预算 5 MB）。显示名 **Tinycast**，bundle `com.tinycast.win`。默认界面语言为简体中文，可在设置里切到英文。

## 功能

启动后常驻托盘，无任务栏按钮。默认 **Alt+Space** 唤出面板（紧凑搜索条，输入后展开）。

| 能力 | 说明 |
|---|---|
| 应用启动器 | 模糊搜索开始菜单、AppX、自定义范围；收藏槽 **Ctrl+1…9 / Ctrl+0** |
| 系统设置 / 系统操作 | `ms-settings:` 页；锁屏、睡眠、音量、深色模式、回收站等 |
| 剪贴板历史 | 文本与图片，可搜索、钉住、粘回唤出前的窗口 |
| 计算器 | 在启动器内联：数学、单位、汇率、日期；Enter 复制并写入历史 |
| 自定义命令 / Quicklinks | 具名命令与链接，可带参数屏与全局热键 |
| 片段 | Markdown 模板、占位符、关键字展开（UIA，失败再 `SendInput`） |
| 窗口管理 | 贴边、三分、虚拟桌面等（默认关） |
| 文件搜索 | Windows Search，不可用则有上限的目录遍历（默认关） |
| Notes | 独立窗口，Markdown 即正文 |
| 日历 | WinRT 日程、入会卡、Join Next（默认关） |
| Emoji | 网格与肤色 |
| 卸载 | 应用行 **Ctrl+K → 卸载**，只进行回收站 |
| AI Chat / Quick Actions | 云端 HTTP（OpenAI / Anthropic / Gemini / OpenRouter / 兼容端点）；无 Apple Intelligence |
| 备份 | 本机设置 round-trip；可尝试导入 Raycast `.rayconfig` |

功能总开关默认关闭的：文件搜索、Notes、片段、窗口管理、日历、AI、Quick Actions、Quicklinks。在 **设置** 里打开后才会出现对应命令和 Tab。

## 要求

- **Windows 11 24H2+**（build 26100+）
- 构建：Rust **stable MSVC**（`x86_64-pc-windows-msvc`）+ Visual Studio 2022 Build Tools（MSVC 与 Windows SDK）

GNU 工具链不支持。不要用 Electron / Tauri / WebView2 去“套一层”。

## 从源码构建

```powershell
git clone https://github.com/YYHCOPPOLO/wincast.git
cd wincast
rustup default stable-x86_64-pc-windows-msvc

cargo test --workspace
cargo build -p tinycast --release
.\target\release\tinycast.exe
```

单实例。已在跑时再启动会退出。退出：托盘菜单 **退出**，或在面板里运行 Quit。

调试构建：`cargo run -p tinycast`。

## 使用

1. 运行 exe。托盘出现图标。
2. **Alt+Space** 唤出面板（设置 → 通用 可改快捷键）。
3. 输入过滤，**Enter** 启动当前行；**Esc** 先清空查询，再关掉面板。
4. **Tab** 在 启动器 → AI Chat（若已开启）→ 剪贴板 之间循环。
5. **Ctrl+K** 打开当前行的 Actions（卸载、别名、快捷键等）。
6. 托盘右键：**设置** / **退出**。设置 → 通用 可切换 **简体中文 / English**（即时刷新，不用重启）。

面板内修饰键按 Windows 习惯：**⌘ → Ctrl**（例如原作 `⌘K` 为 `Ctrl+K`）。

### 字体与实验性模糊

- 拉丁文字用 **Segoe UI Variable**（按字号选 Small / Text / Display 光学尺寸；未安装时回退 Segoe UI），中日韩文字经 DirectWrite 字体回退落到 **Microsoft YaHei UI**。搜索框等系统 `EDIT` 控件暂时仍用 YaHei UI，以保证输入法正常。
- 面板背后的真实模糊默认关闭。启动前设置环境变量 `TINYCAST_EXPERIMENTAL_BLUR=1` 可试用（需要 Windows 11 并开启“透明效果”）；任何条件不满足时自动退回现在的纯遮罩。

## 架构

```
crates/
  tinycast-pure/   决策层：搜索、计算器、布局、文案表。禁止依赖 windows
  tinycast/        Effect + AppCore + Direct2D 绘制。唯一 exe
```

UI 线程一条。扫应用、解码图、FTS、HTTP 流等进线程池，结果 `PostMessage` 回面板。绘制用 Direct2D / DirectWrite（默认字体 Microsoft YaHei UI）；搜索框与 Notes 用系统编辑控件，不手写 IME。

## 数据位置

| 内容 | 路径 |
|---|---|
| 设置、Chat、片段、学习排序 | `%APPDATA%\com.tinycast.win\` |
| 剪贴板库、汇率缓存 | `%LOCALAPPDATA%\com.tinycast.win\` |
| API Key | DPAPI 保护，按 connection UUID 分条，不进备份 |

开机启动（若打开）：`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`。

## 明确不做（v1）

- **Raycast 扩展运行时**（设置里有 Extensions 页，打开时说明尚未交付，不会塞 JS 引擎）
- **本机 Apple Intelligence / Foundation Models**
- 没有公开 API 的系统能力（例如 Stage Manager）；目录里仍保留该命令，执行时 HUD 说明不可用

## 许可

本仓库 `Cargo.toml` 声明 **MIT**。这是独立重写：对照 Tinycast 上游 main（`da2c4e44`，此前为 v0.10.2）的公开文档与 Theme token，不复制其 Swift 源码。原作本身是 [AGPL-3.0](https://github.com/abue-ammar/tinycast)。
