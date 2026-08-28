# Tinycast for Windows — 产品设计

- **日期：** 2026-08-28
- **上游对照：** Tinycast `v0.10.2`（tag commit `7a79585`）
- **性质：** 独立重写。复用公开文档与 Theme token 数值，不复制 Swift 源码。
- **身份：** bundle `com.tinycast.win`；显示名 Tinycast

## 1. 行为神谕

用户可见行为（面板、命令名称与英文字符串、分区、设置侧栏、快捷键语义、确认门、备份排除项、HUD/对话框规则）以 **Tinycast v0.10.2 的 `docs/` 与 shipped catalog** 为准。文案不另译成中文；「Show in menu bar」等原句保留，托盘只是 Effect。「Restart your Mac?」这类会话结束确认改为 PC。

本文件只记录：Windows 重写的架构、技术栈、Effect 层替换，以及明确的产品切口。本文件与 v0.10.2 文档在用户可见行为上冲突时，**v0.10.2 赢**，除非落在第 2 节的切口或第 8 节的 Windows Effect 契约里。

实现时读原作对应 `docs/features/<name>.md`，不要在本文件里复述每一条不变量。

## 2. 产品切口（v1）

v1 是功能对齐的第一版，不是单独的面板壳。

| 项 | 决定 |
|---|---|
| 对齐 | 原作功能与 UI |
| 后做 | Raycast **扩展运行时**（设置页、`PaletteMode.extensionCommand`、备份排除项仍按原作留下） |
| 不做 | Apple Intelligence / 本机 Foundation Models。AI 设置里不得出现该路由或凭据栏 |
| 做 | HTTP 云端 Chat（API Key、流式、UI 对齐原作 Chat）；Quick Actions 四条走同一 provider |
| 预算 | Release 5 MB、关面板后 RSS 100 MB 是要挤的目标。超了必须解释并继续压，**不为数字砍功能** |

ChatGPT 订阅路由（原作 `codex app-server`）保留：本机 PATH 上有 `codex` 才 Connect；没有则设置页按原作链到安装说明，不静默改走 API。

## 3. 技术栈

| 层 | 选择 |
|---|---|
| 语言 | Rust，`x86_64-pc-windows-msvc`，Windows 11 24H2+（26100+） |
| 产物 | 一个 exe，不带运行时 / WebView / 自包含 .NET |
| 窗口 | Win32 HWND：托盘、tool window 面板、设置、对话框、HUD、Notes、摄像头预览 |
| 绘制 | Direct2D + DirectWrite；系统 Acrylic + scrim；失败则 `UpdateLayeredWindow` 位图路径 |
| 输入 | 搜索框与 Notes 宿主真正的 `EDIT` / TSF / RichEdit，不手写 IME |
| 绑定 | 官方 `windows` crate（Win32、DWM、UIA、Shell、WinRT） |
| 存储 | `rusqlite` + bundled SQLite（剪贴板、Chat 历史） |
| 序列化 | `serde` / `serde_json` |
| 文本 | `unicode-segmentation`（片段光标按字素） |
| HTTP | WinHTTP（或同等 `windows` 绑），**ephemeral、无 URL cache**。不引入默认功能集很肥的 HTTP crate，不引入 tokio 运行时 |
| 密钥 | DPAPI `CryptProtectData`，按 connection UUID 分条，不进备份、不进日志 |
| 图标 | `IShellItemImageFactory` / `ExtractIconEx`；字形 Segoe Fluent Icons。SF Symbol 名经一张映射表落到 Fluent / 捆绑资源（蓝牙商标等与原作一样走资源，不走系统符号） |

**禁止：** Electron、Tauri、WebView2、WinUI 3、WPF 主壳、winit、wgpu、egui、iced、tao、wry、V8。QuickJS 只在扩展运行时阶段、磁盘仍剩 ≥ 1.2 MB 时再单独立项。

Release profile：`lto = true`、`codegen-units = 1`、`opt-level = "s"`、`panic = "abort"`、`strip = true`。`windows` crate 只开用到的 features。

DPI：Per-Monitor V2。token 单位是 DIP（96 dpi 独立像素 ≡ SwiftUI point）。

## 4. 架构

四层，编译卡住边界：

1. **Pure** — crate `tinycast-pure`，**禁止依赖 `windows`**。模糊搜、band/frecency、计算器、片段/Quicklink 模板、窗口几何、卸载规则、会议窗口、Chat 消息模型、`PaletteRowIndex`、`PalettePlacement`。时钟、文件系统、网络、区域货币全部注入。
2. **Effect** — Win32 / WinRT I/O。一个功能一个文件夹。确认门不在 Runner 里。
3. **AppCore** — 唯一长寿状态，只活在 UI 线程。`start()` 是唯一启动接线。功能动作在该功能的 Coordinator；视图不得越过 coordinator 去 mutate store。
4. **View** — Direct2D 只画。不决策。

第一刀只拆 `tinycast-pure` + `tinycast`（bin：Effect + AppCore + View）。功能目录对齐原作：`features/<name>/{model,service,ui,settings}`。`design_system/` 与 `platform/` 不得依赖功能。

并发：UI 线程一条。重活（扫应用、解码图、FTS、HTTP 流、卸载计量）进 Windows 线程池，结果 `PostMessage` 回面板 HWND。不引入第二套 actor。

命名后缀对齐原作 `docs/standards.md`：Store / Coordinator / Controller / Runner / Catalog / Engine / Policy / Session / Index。

## 5. 进程壳与面板

常驻托盘附属进程，无任务栏按钮（tool window + 可激活，否则 IME 失败）。单实例。未打包开机启动：`HKCU\...\Run`。

唤出热键默认 **⌥Space → Alt+Space**（原作 Carbon 默认）。运行时会盖住窗口系统菜单；设置里可改。面板内修饰键：**⌘ → Ctrl**（`Ctrl+K` Actions、`Ctrl+1…0` 收藏槽）。KeyCap 语言与原作同一套，字形按 Windows 习惯。

面板：无边框置顶。锚点每轮召唤解析一次，紧凑↔展开只改高度、顶边不漂。工作区高度 **18%** 处向下长。紧凑 64 DIP，展开 750×475，圆角 26。Dark scrim 黑 0.40；Light 白 0.55。顶栏 44 / 底栏 52 是浮层。边缘溶解，列表铺满。进入 180 ms / 退出 120 ms，scale 0.94。失焦隐藏。自己画对话框和 HUD，不用 `MessageBox`。占位符自己画，组字时用 `isComposing` 藏掉。

空闲托盘目标 15–40 MB；面板打开 < 40 MB；峰值 < 80 MB；硬顶目标 100 MB。图标缓存 8 MB，键 = 路径 + mtime + DPI + 外观世代，**面板关掉就丢**。验证必须在浅色壁纸上做。

Acrylic + 分层 HWND + 圆角：先系统模糊；裁成方块则改位图路径，不改 token。

设置窗 860×700，侧栏 215，详情列最小 420。Notes、Onboarding、Support、更新说明、摄像头预览都是独立 HWND，生命周期与面板互不影响。

Theme 数值以原作 `Tinycast/DesignSystem/Theme.swift` 为唯一源。Dark 分支是冻结字面量。新颜色只走 ramp / adaptive。禁止表面实心灰。

## 6. 命令表面

所有可搜条目是带 **Kind** 的 `AppEntry`。Kind 同时决定分区、Visibility、设置页。分类总开关关掉：行与该分类热键一起停；单项勾选只藏行。

空查询分区顺序：

**Favorites（若有）→ Applications → System Settings → Quicklinks → Snippets → System Actions → Window Management → Custom Commands → Commands**

Favorites 占用 Ctrl+1…9 / Ctrl+0。

查询分字段 band 与 `bandStride` / `FuzzyMatch.maximumScore` / `LauncherRankingStore.maximumBoost` 合同对齐原作：频次不能跨档。分类名触发是 **精确相等**。用户别名从开头的精确/前缀走最高档。

内联计算器卡片在有查询且引擎有结果时占 selection 0。入会卡只在空查询。两张卡不同时出现。

**Tab 环：启动器 → AI Chat → 剪贴板 → 启动器**（`aiEnabled` 关则只剩启动器 ↔ 剪贴板）。其它模式从命令或热键进入，Esc 先清空查询再退出。卸载只从应用行 Actions 进入。Ctrl+K 是当前行 Actions。

`PaletteMode` 对齐原作：launcher、clipboard、calculatorHistory、emoji、fileSearch、schedule、uninstall、quicklinks、quicklinkArguments、ai、aiHistory；`extensionCommand` 占位，v1 不进入。

内建 `CommandID` 与原作同 raw value、同显示名、同 `hotKeyAction` 集合。功能开关关掉则对应命令不发布。

没有 PowerToys 式 `>` 前缀语法。

## 7. 设置

侧栏分组声明顺序即显示顺序：

| SettingsSection | SettingsTab |
|---|---|
| General | General, Permissions |
| Launcher | Applications, System Settings, System Actions, Commands, Quicklinks |
| Features | AI, Quick Actions, File Search, Notes, Snippets, Window Management, Clipboard, Emoji & Symbols, Calendar, Extensions |
| Advanced | Backup, About |

页 ID 用枚举自身，不用下标。Applications / System Settings / System Actions / Commands 共用 `LauncherItemsSection`（可见性、别名、快捷键记录器）。没有单独 Shortcuts 页。

General 区块与原作相同：Global Shortcuts、Search（重置学习排序）、Hyper Key、Appearance、General（Launch at login、托盘图标＝原作 Show in menu bar、Pop to Root、打开面板时切换输入法）。

Permissions 列出 Windows 对应能力（UI Automation / 低级键盘钩、日历、摄像头、剪贴板图），只打开系统设置，启动时不弹授权。真正的 prompt 只来自打开该功能的那次手势。

`AppSettingsKey` raw value 与原作相同，以便备份字段名稳定。存储为 Roaming 下 JSON（Windows 没有 UserDefaults）。覆盖表抄原作 `SettingsBackupCoverage`：能力开关与 AI 全套、日历授权、摄像头、自动入会、片段开关、扩展开关/注册表/路径、面板位置、输入法 ID **不得**进备份。

## 8. 功能对齐与 Windows Effect

用户能做的事对齐原作。下面只写 Effect 替换。Mac 上没有 Windows 对等物的命令：**目录里仍在**，执行走 HUD 说明不可用，不改名、不删行、不偷偷映射成另一个产品。

### 8.1 启动器

- 扫描：开始菜单 `.lnk`、AppX `PackageManager`、`App Paths`、用户 `searchScopes`（tilde 风格改成 `%USERPROFILE%` 缩写，备份可移植）。
- 深度规则对齐原作：作用域下一层子文件夹，把 `.lnk` / AppX 当叶子，不递归进安装目录深处。
- 别名：`.lnk` 参数、AppX 显示名、VERSIONINFO 文件描述；过滤「等于显示名」和占位垃圾，避免 `app` 命中全表。
- System Settings 分区：Windows 设置页（`ms-settings:` 等），不是 `.appex`。

### 8.2 剪贴板

`AddClipboardFormatListener`，不用 0.5s 轮询。自写入打私有格式标记。SQLite + FTS5 trigram（满 3 字符才 FTS）、钉住、类型过滤、Ctrl+P 菜单。图片文件在 Local 缓存，内存只留缩略图，窗口 1000 条。密码管理器窗口排除。UWP 延迟内容可能漏一次捕获，属契约内。

### 8.3 计算器

Pure 引擎对齐原作文档。区域货币来自 Windows 区域设置。汇率 HTTPS、24h 快照、cacheless session。Enter 复制并写入 Calculator History。

### 8.4 自定义命令 / Quicklinks / Emoji

行为、占位符、参数屏对齐。自定义命令 `CreateProcess`；确认在 Coordinator。Quicklink 打开用 `ShellExecute`。Emoji 网格与肤色设置对齐；字体用 Segoe UI Emoji。

### 8.5 片段

Markdown 库路径：`%APPDATA%\com.tinycast.win\Snippets\`。token、关键字、确认 HUD、备份排除 `snippetsEnabled` 对齐。插入：UIA `ValuePattern`/`TextPattern`，失败再 `SendInput`。LL 钩 **仅功能打开时装**。被提升窗口、部分浏览器/游戏全屏：拒绝，不猜，不改已键入关键字。

### 8.6 系统操作

`SystemAction.ID` 全表保留。Win32 / WASAPI / 电源 API 能做的做。确认文案里的 “Mac” 改为 “PC”，其余名称不变。

Windows 无对等物、执行时 HUD 说明不可用：

- `toggle-stage-manager`

其余按公开 API 做（锁屏、睡眠、显示器睡眠、重启/关机/注销、屏保、媒体键、音量 5% 格、显示桌面、深色模式、回收站、弹出可移动磁盘、隐藏文件、隐藏其它窗口、退出其它进程、关闭通知、蓝牙）。Hide / Quit All 目标是唤出前的前台窗口。提升后的窗口可能动不了。没有公开 API 的不做私有挂钩。

### 8.7 窗口管理

34 条 `WindowCommand.ID` 全表、间隙规则、循环、Restore 的 Pure 几何对齐。Effect：`SetWindowPos` + DWM。虚拟桌面用 `IVirtualDesktopManager` 等公开 API，**不伪造触控板手势**。坐标空间：布局在顶左原点 DIP 中计算，写入前转到屏幕像素；多显示器锚在主屏，禁止用窗口所在屏高度做翻转。功能默认关。提升窗口失败则静默（原作「失败要安静」）。

### 8.8 文件搜索

默认关。文件名、空查询不干活、1000 候选 / 200 行。Windows Search OLE DB；不可用则同样上限的目录遍历。隐藏路径与安装包内部结构仍排除。不索要「完全磁盘访问」式权限。

### 8.9 Notes

独立标题窗、Markdown 源即正文、无预览/无任务勾选。宿主系统编辑控件。frame 由用户拖，系统记住。

### 8.10 日历

五条命令、空查询入会卡、Join Next、托盘摘要、自动入会、摄像头预览对齐。数据：WinRT Appointments。没日历或未授权则空，不编造。`calendarEnabled` / auto join / camera 不进备份。入会链接检测是 Pure，与原作同一套 provider 表。

### 8.11 卸载

从应用行 Ctrl+K → Uninstall Application 进子屏。只进行回收站，永不永久删除。先 discover 再异步 measure。根表换成 ARP、AppX、常见残留目录（Roaming/Local/ProgramData 下按 bundle / 显示名规则）。匹配与锁定是 Pure。拒卸自己（按运行中的 `com.tinycast.win`）。需要管理员的项锁定，不弹 UAC。

### 8.12 热键

`HotKeyBinding`：`.combo` 走 `RegisterHotKey`；`.doubleTap` 走 LL 钩且 **仅有绑定时安装**。Hyper：Caps Lock 用 HKCU Scan Code Map 映到空闲扫描码，退出和关掉功能时清除，不在重启后残留。右侧修饰键作 Hyper 不改 Scan Code Map。Include Shift 重指向已录制和弦，对齐原作。记录器不是可聚焦控件：设置窗本地钩，全局引擎暂停。

### 8.13 AI / Quick Actions

Chat 是 palette 屏。流式、`ai-chats.sqlite3`、Opens to、Keep conversations、composer=搜索框、Send/Stop ↵、Ctrl+K → New Chat / History / AI Settings。无 Apple Intelligence 路由；不可用本机模型时不得改走云。HTTPS；localhost 才允许明文。Key 在 DPAPI。关闭 AI = 无命令、无 Tab 站、不打开库、取消流；已存对话文件不动。

Provider 预设：OpenAI、Anthropic、Gemini、OpenRouter、OpenAI Compatible（base URL 可改）。模型列表按原作：发现 + 手填，不内置会过期的目录。

Quick Actions 四条（Fix Grammar / Rewrite / Translate / Summarize）对齐原作：读前台选区，可选预览面板，写入走与片段相同的投递栈。`quickActionsEnabled` 不进备份。

### 8.14 Backup 与 Raycast 导入

Tinycast 自身备份 round-trip 以同一 build 为准。Import from Raycast 保留 `.rayconfig` v1/v2 解码；Windows 上常常没有源，失败要说清楚，不得把错误口令报成格式错误。导入不得打开片段监听或扩展运行。

### 8.15 Extensions

设置页、可见性开关、备份排除项按原作留下。v1 打开页时说明运行时未交付；开关打不开运行时。不得在 v1 塞 JS 引擎。

### 8.16 更新 / Support / Onboarding

Check for Updates：GitHub Releases of **本 Windows 仓库**，zip 自更新，逻辑对齐原作 updates.md（通道按 bundle、dev 不更新、24h 检查、安装前校验）。仓库尚未发布时，命令 HUD「尚未配置更新源」，不崩溃。Support / About / 30 天提醒 / Onboarding 窗口对齐原作路由。

## 9. 数据位置

| 原作 | Windows |
|---|---|
| `~/Library/Application Support/<bundle>/` | `%APPDATA%\com.tinycast.win\` |
| `~/Library/Caches/<bundle>/` | `%LOCALAPPDATA%\com.tinycast.win\` |
| UserDefaults | `%APPDATA%\com.tinycast.win\settings.json`（键名 = `AppSettingsKey` raw value） |
| Keychain | DPAPI 文件，按 connection UUID |
| 剪贴板 sqlite + 图片 | Local 下 `clipboard.sqlite3` + `images\` |
| Chat sqlite | Roaming 下 `ai-chats.sqlite3` |
| 片段 Markdown | Roaming `Snippets\` |
| 学习排序 | Roaming `launcher-ranking.json` |
| 汇率快照 | Local `currency-rates.json` |

## 10. 数据流

```
HotKey / Tray / Protocol
        │
        ▼
AppCore.start() 唯一接线
        │
        ├─► Coordinator（确认、enable 门）
        │         │
        │         ▼
        │    Runner / Store / Monitor   （Effect，可下线程池）
        │         │
        │         ▼
        │    PostMessage → UI 线程 → Store 发布
        │
        └─► PaletteState / Settings / 其它 HWND
                  │
                  ▼
             Direct2D View（只读状态）
```

召唤路径解析一次：previousApp（粘贴/窗口管理目标）、屏幕锚点、输入法、PasteTarget。禁止每帧解析。

Pop to Root：隐藏后按超时回到 launcher，并按 AI Opens to 决定是否新开对话；流式回复进行中不丢。

## 11. 错误处理

- `panic=abort` 只留给编程错误。Effect 失败是状态。
- 用户必须确认或知情：自绘 Dialog（图标=主体符号，tone 与按钮角色三轴独立，Enter 执行、Esc 取消、Cancel 在左）。
- 短暂结果：Message HUD 药丸；音量走 Volume HUD。
- 无事可做是结果不是失败（空回收站、无可弹出磁盘），HUD `.neutral`。
- 不可用的平台命令：HUD 一句话，不弹对话框。
- HTTP / AI：部分文本保留，错误进对话气泡，不把 provider 调试串给用户。
- 日志用结构化 logger；禁止 `print` 当 UX。

UAC：未做 UI Access。提升窗口上的粘贴、热键、贴边失败则按上面规则报告或静默（窗口管理静默）。

Defender：LL 钩只在片段 / Hyper / 双击修饰键实际需要时装。

## 12. 测试门

`tinycast-pure` 的测试编译 **shipped 源**，不抄一份。至少覆盖原作 harness 对应的纯逻辑：

- SearchRelevance / ranking / scopes
- CalcEngine
- PaletteRowIndex / PalettePlacement
- WindowLayout / WindowActionMemory
- UninstallRules / UninstallSelection
- Snippet 模板与 Markdown codec
- MeetingLink / UpcomingWindow / AutoJoinPolicy
- AI endpoint policy、stream 分帧、ChatSession 边界
- SettingsBackupCoverage：每个 `AppSettingsKey` 恰好落在 mirrored / external / excluded 之一
- DoubleTapDetector

Effect 用假时钟、假 FS、假 HTTP。UI 像素不以截图当门；面板壳用手动清单：浅色壁纸、圆角、IME 组字、紧凑展开顶边不漂。

定义完成：`cargo test`（pure + 带 mock 的 coordinator 门）通过；Release 体积与关面板 RSS 记入分账（超标解释，不删功能）。

## 13. 实现切片（仍是同一 v1）

写作计划可按依赖切开，但合并前 v1 范围内功能都要在。建议顺序只约束依赖，不构成产品分期：

0. workspace、pure crate 门、Theme token、消息循环、托盘、面板壳、EDIT、热键唤出、设置窗骨架
1. AppIndex + 模糊搜 + 启动
2. 剪贴板 + 计算器
3. 自定义命令、Quicklinks、Emoji、片段（先面板展开再关键字）
4. 系统操作 + 窗口管理
5. 文件搜索、Notes、日历、卸载、Backup、Onboarding、Support
6. HTTP AI + Quick Actions；更新检查
7. Extensions 页占位（无运行时）

每刀结束后量一次 exe 与 RSS。

## 14. 明确不做（v1）

- 复制 Tinycast Swift / 把 Swift 编到 Windows
- Apple Intelligence
- Raycast 扩展运行时（QuickJS/V8）
- 为过 5 MB 删除第 2 节范围内的功能
- WebView 面板
- 兼容 Windows 10
- 未签名 UI Access
- 伪造 HID 切虚拟桌面
- 卸载永久删除
- 启动时安装键盘钩或弹授权
