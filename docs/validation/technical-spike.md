# NxMonitor 最小技术验证报告

## 范围与结论

本轮交付的是已确认需求的本地基线、真实外部 PID 采集和原生透明 HUD 原型。**不是完整首版交付，也不是全平台兼容承诺。**

结论：在当前 Windows 10 22H2 x64 普通权限会话中，GPUI-Kit 0.8.0 可构建并显示原生 HUD；PresentMon 2.6.0 可非注入采集外部 Roblox Studio 的真实呈现帧；透明、置顶、点击穿透、编辑模式、重锁、全局退出和提前关闭采集的清理路径均完成受控测试。

## 环境与依赖

- OS：Windows 10 Pro，10.0.19045，x64。
- 当前 token 的管理员角色检查为 false；未请求 UAC、未调整用户组。
- Rust/Cargo 1.96.0，`stable-x86_64-pc-windows-msvc`；Visual Studio 2022 Build Tools 17.14，VC x64 工具已安装；CMake 可用。
- 固定 `gpui-kit = 0.8.0`，匹配 `gpui-pre-* = 0.3.9`；默认后端，无 Fast。
- Intel PresentMon 2.6.0 x64：SHA-256 `b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af`；Authenticode 状态 Valid，签署方 Intel Corporation。
- `Cargo.lock` 已生成。此处核对直接依赖来源与许可，不声称已完成所有传递依赖/素材的发行审计。

## 一手资料与选型依据

### GPUI-Kit

- [安装指南](https://gpui-kit.com/zh-CN/docs/installation/)：Windows 10+、VS2022 C++/SDK、MSVC、CMake；该页建议 Rust 1.92+，本机工具链更高。
- [入门指南](https://gpui-kit.com/zh-CN/docs/getting-started/)：公开 `gpui-kit = 0.8.0`，通过 `gpui_kit` facade 初始化与创建 Root 包裹窗口。
- [crate 元数据](https://crates.io/crates/gpui-kit/0.8.0)、[源仓库](https://github.com/longbridge/gpui-kit)、[Apache-2.0](https://github.com/longbridge/gpui-kit/blob/main/LICENSE-APACHE)、[文档许可](https://github.com/longbridge/gpui-kit/blob/main/LICENSE-DOCS.md)。归档声明见 [THIRD_PARTY_NOTICES](../../THIRD_PARTY_NOTICES.md)。
- 实际编译前检查已下载包源码：`gpui-kit/src/lib.rs` 的 `init/open_window`；`gpui-pre/src/platform.rs` 的 `WindowOptions/WindowBackgroundAppearance/WindowKind`；`gpui-pre/src/window.rs` 的 `HasWindowHandle`；`gpui-base/src/root.rs` 的 Root `Styled/update`；`gpui-component/src/root.rs` 的默认 Root background；`gpui-pre-windows/src/window.rs` 的 Popup 和透明平台实现。未修改第三方源码。

### PresentMon

- [v2.6.0 官方 release](https://github.com/GameTechDev/PresentMon/releases/tag/v2.6.0)、[资产](https://github.com/GameTechDev/PresentMon/releases/download/v2.6.0/PresentMon-2.6.0-x64.exe)。体积约 980KB，不安装完整 MSI。
- [README 权限与 ETW 说明](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/README.md#L9-L65)、[MIT 许可](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/LICENSE.txt)。普通账户可能需要 Performance Log Users 身份；跨账户或短命进程名可能 unknown。本机成功不代表任意标准账户都能成功。
- [CLI 文档](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/README-ConsoleApplication.md)：PID 过滤、stdout、定时终止与每帧字段。
- [CSV 源码](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/PresentMon/CsvOutput.cpp#L563-L608)：默认和 v1/v2 字段不同；本原型显式 v1，并据真实输出兼容 `msBetweenPresents` 字段。stdout 逐行 flush 路径见 [1335–1349](https://github.com/GameTechDev/PresentMon/blob/v2.6.0/PresentMon/CsvOutput.cpp#L1335-L1349)。
- [Microsoft StartTraceW](https://learn.microsoft.com/en-us/windows/win32/api/evntrace/nf-evntrace-starttracew#return-value)：权限约束，不把不提权运行等同于无权限要求。

呈现 FPS 来源是 `Present()` 间隔；不是显示器刷新率、GPU耗时倒数或 NxMonitor 自身刷新率。显示 FPS、帧生成及 dropped frame 的产品语义需要后续另行设计。

## 本轮实现

- `src/presentmon.rs`：按列名解析、时间列携带单位、兼容 BOM/大小写、无效值过滤。
- `src/stats.rs`：交换链隔离、活跃链启发式、30s滚动/会话统计、1秒桶极值、尾部 low、历史帧间隔。
- `src/capture.rs`：单 PID、有界会话、唯一 ETW 名称、后台 stdout/stderr 消费、错误/完成状态、关闭时停止自己的会话并等待 helper。
- `src/native.rs`：自有 HWND 的置顶/穿透切换，独立线程注册/消费全局热键，冲突时尝试备用组，生命周期结束释放热键。
- `src/bin/hud.rs`：双语初步预设、真实读数与历史图、过期/不可用状态、编辑拖动、错误首屏和完整诊断复制。
- `scripts/Get-PresentMon.ps1`：显式下载，固定 hash 和 Intel 签名验证，不自动执行安装程序。
- `scripts/Test-Overlay.ps1`：受控桌面回归测试；临时文件写入指定输出目录/会话 scratch，不点击现有用户应用。

## 验证结果

### 自动检查

- `cargo build --locked --features hud --bins`：通过。
- `cargo test --locked --features hud --all-targets`：12 个核心测试通过；两个 bin 无额外测试用例。
- `cargo clippy --locked --features hud --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`：通过。
- 单次只读代码审阅指出的时间单位、零帧完成、错误不可见问题已修复并重测；未进行反复扩张式审阅。

### 外部真实采集

- 目标：正在运行的外部 `RobloxStudioBeta.exe`，测试时 PID 21444；无需更改其代码或注入。
- HUD 连接复测曾收到 936 条；项目内最终桌面脚本复测收到 392 条，rejected=0，error=None。不同测试样本数与瞬时值波动正常，不作为性能基准。
- CLI 实采一次5秒接收1100条有效帧；最终4秒复测接收856条。同一PID多交换链分开统计。
- 非渲染目标零帧验证：PresentMon 可完全不输出表头；空流按无目标帧处理。不能把它显示成 0 FPS 或正常统计完成。

### 原生窗口受控测试

- 创建灰色 WinForms 测试窗口，HUD 位于其前方；从实际 HWND rect 推导测试坐标。
- 测量可见像素确认实际绘制。Root 默认不透明导致原始 alpha 无效，已通过公开 Styled 覆盖此 HUD Root 背景。现代约0.9、Graphy约0.68透明度在测试表面产生不同混合像素。
- `WS_EX_TOPMOST` / `WS_EX_LAYERED` / `WS_EX_TRANSPARENT` 状态正确；Windows hit test 和实际鼠标点击确认穿透时底层窗收到一次点击，编辑时不泄漏点击。
- 全局编辑、重锁、退出均通过；默认 Ctrl+Alt+E 曾与现有程序冲突，改为可回退功能键组并显示实际注册组合。
- 早期截图在点击底层 TopMost 窗后采集，底层窗被提升导致 HUD 看似消失；调整为点击前截图并以像素/命中共同验证，排除误判。
- 独立对比：去掉 layered 后跨进程穿透失败，因此保留已验证组合。不把样式位存在单独当作行为通过。
- 提前退出后 PresentMon 子进程消失；`logman query -ets` 未发现 NxMonitor 会话。测试未终止其他 ETW 会话。
- 截图：[英文现代窗口](overlay-window.png)、[中文 Graphy 真实采集](overlay-live-zh.png)、[证据页](overlay-evidence.html)。截图仅覆盖测试窗口区域，不是最终设计验收。

## 未验证及仍待实现

- Windows 11、真实游戏的普通/无边框模式、独占全屏、反作弊兼容、多屏与不同 DPI、屏幕读者和完整键盘流程尚未实测。
- 测试窗口穿透与外部 Studio 采集成功，不等于任意游戏 overlay 支持通过。
- 不支持任意旧 Windows10 版本的已验证承诺；仅19045已测。
- 原型每链200万样本上限、最多600秒；高于约3333FPS的600秒会话可能达到上限，拒收可导致过期。不是最终低开销/无限会话架构。
- 主交换链启发式可能在多窗口应用中切换；需后续添加更稳定选择/用户控制。
- 无音频、内存、设备信息、自动前台/黑名单、完整设置、托盘/开机启动、多屏/持久化等完整首版功能。
- 未发布二进制、未配置安装器或CI、未初始化Git。项目自身开源许可及完整传递许可审计待公开分发前确定，不阻塞本地验证。

## 下一步建议

在这个可运行基础上增加目标选择状态机与黑名单，再建设独立设置窗口与持久化；随后接入系统/目标进程内存和音频。Windows11、无边框全屏和多屏实机验证应与这些工作并行，而不是以当前窗口测试替代。
