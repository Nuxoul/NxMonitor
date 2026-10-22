# NxMonitor

Rust + GPUI-Kit 的 Windows 性能 HUD。**当前已完成目标选择与设置闭环，仍不是完整首版。**

已实现：自动/手动目标、黑名单、独立双语设置与配置恢复、真实FPS与内存监测、基础设备/显示信息、透明置顶、穿透/编辑、采集重置与退出清理。

尚未实现：音频、托盘、开机启动、多屏选择、完整外观持久化、无限会话和完整游戏识别。正式HUD尚未完整整合静态Graphy视觉。[确认后的需求](docs/requirements.md)不变。

内存每秒后台采样：系统已用/总物理显示GiB，目标工作集显示MiB（非私有内存）。无权限/退出/过期不显示旧值为正常读数。[内存验证](docs/validation/memory.md)。

设备详情在设置窗口；HUD显示目标屏幕和客户区物理尺寸。GPU列表不代表目标正在使用的GPU，刷新率不是FPS。[设备验证](docs/validation/device.md)。

## 开发交接与文档

换电脑或让新的开发助手接手时，先读 [当前状态与下一步](docs/STATUS.md) 和 [跨电脑开发指南](docs/DEVELOPMENT.md)，无需依赖聊天记录。

| 文档 | 内容 |
| --- | --- |
| [功能说明](docs/PRODUCT.md) | 产品范围、指标口径、已实现与待实现功能 |
| [整体任务路线](docs/ROADMAP.md) | 带任务ID、优先级、依赖和验收标准的开发清单 |
| [当前架构](docs/ARCHITECTURE.md) | 模块职责、线程/资源所有权、配置schema与关键约束 |
| [开发与GitHub同步](docs/DEVELOPMENT.md) | 新电脑搭环境、构建、运行、隔离设置与上传注意事项 |
| [测试与兼容矩阵](docs/TESTING.md) | 自动检查、原生回归步骤、平台已测/未测边界 |
| [开发助手约定](AGENTS.md) | 接手时的阅读顺序、不可破坏的规则与检查命令 |

安全开发配置示例：[examples/settings.example.json](examples/settings.example.json)。复制到已忽略的 `local/` 后使用 `--config`，不要提交个人配置、采集日志或构建产物。

## 环境

- Windows x64；本机验证为 Windows 10 22H2（19045）。Windows 11、无边框全屏和独占全屏尚未实测。
- Rust 1.96+ MSVC 工具链、Visual Studio 2022 C++ Build Tools + Windows SDK、CMake。
- 当前固定 `gpui-kit = 0.8.0`，使用默认上游 GPUI 快照，不启用 Fast。

## 构建与运行

```powershell
# 若本机已装 Rust，但当前终端找不到 cargo：仅补充当前进程 PATH。
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

cargo build --locked --features hud --bins

# 显式下载官方 PresentMon，核对固定 SHA-256 与 Intel 签名。
.\scripts\Get-PresentMon.ps1

# 推荐入口：打开 HUD 和独立设置窗口，无需手填 PID。
.\target\debug\nxmonitor-desktop.exe

# 隔离配置测试，不影响默认设置。
.\target\debug\nxmonitor-desktop.exe --config C:\path\nxmonitor-test.json

# 以下 nxmonitor-hud 仍是保留的技术验证入口。

# 查找目标 PID；不要使用 0，也不要选择 NxMonitor 自身。
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 } |
    Select-Object Id, ProcessName, MainWindowTitle

# 将 12345 换成真实外部图形应用 PID。
.\target\debug\nxmonitor-hud.exe --presentmon .\tools\PresentMon-2.6.0-x64.exe --pid 12345 --seconds 60 --lang zh --style graphy

# 英文现代预设
.\target\debug\nxmonitor-hud.exe --presentmon .\tools\PresentMon-2.6.0-x64.exe --pid 12345 --seconds 60 --lang en --style modern

# 不启动采集器，仅验证原生窗口；读数为空而不是演示数据。
.\target\debug\nxmonitor-hud.exe --window-only --smoke-seconds 10

# 不依赖 UI 的采集探针
cargo run --locked --bin nxmonitor -- --presentmon .\tools\PresentMon-2.6.0-x64.exe --pid 12345 --seconds 15
```

首次图形依赖构建可能需要几分钟。Debug 构建未作性能优化，不能作为最终开销基准。

### HUD 操作

- 默认鼠标穿透，不抢焦点。
- 默认 `Ctrl+Alt+F6` 进入/退出编辑模式，`Ctrl+Alt+F7` 退出程序。
- 冲突时尝试 F8/F9、F10/F11 两组；**以 HUD 显示及控制台 `HOTKEYS` 日志为准**。全部冲突则拒绝启用穿透；桌面入口保留设置/退出路径并显示错误，旧验证入口会结束。
- 编辑模式可拖动标题整体移动，也可点击锁定/退出按钮。
- 单次采集限制 1–600 秒；到期保留统计，当前实时读数显示不可用。窗口可继续保留，直到主动退出。
- 技术验证入口 `nxmonitor-hud` 有错误诊断复制；桌面入口 `nxmonitor-desktop` 的采集诊断在设置中查看。按钮操作前先解除HUD穿透。
- 不自动提权、不安装服务/驱动、不修改用户组。权限不足时依据诊断自行选择授权，不要为兼容游戏绕过安全限制。

### 目标与设置

- 默认设置文件：`%LOCALAPPDATA%\NxMonitor\settings.json`。语言、预设、模式、黑名单、采集器路径会保存。
- 自动模式跟随有可见窗口且可读取身份的前台应用，稳定约650ms后切换；不是所有游戏/启动器识别器。切换到 NxMonitor 自己的设置窗口时保留有效目标。
- 黑名单支持文件名/完整路径精确匹配，不区分大小写并规范路径分隔符，不支持通配符；只影响自动选择。
- 手动选择黑名单目标需显式确认。手动锁定绑定 PID + 启动时间，防止 PID 复用；切回自动后旧锁定清除。
- 重启仅恢复手动目标的路径提示，不自动恢复 PID，需重新选择。
- 设置里的“重置并重新采集”开启新会话；目标变化清空旧统计，切换清理在后台完成。
- 独立设置窗口可关闭；HUD 编辑模式中的“设置…”可重新打开。设置窗口的“退出 NxMonitor”结束整个应用。
- 配置损坏时保留原文件、禁止覆盖且不自动采集；修正/重命名原配置后重启。支持 UTF-8 BOM。
- 默认单目标会话仍限制600秒，到期从设置重启；不是完整无限监测版本。

## 数据口径与原型限制

- 当前使用 `MsBetweenPresents` / v1 `msBetweenPresents`：**presented FPS，不是 displayed FPS**，不表示每帧都实际上屏。
- 同一 PID 多交换链分别统计，以最近一秒活动样本最多的交换链作为当前显示链。它是原型启发式，未等同于游戏主交换链识别；切链时不合并两条链的统计。
- 当前 FPS / ms 为该链最近一秒有效帧间隔的聚合；avg 为样本数 / 总帧时间。
- 默认滚动 30 秒；另计算当前显示链的会话统计。首个原型只显示会话平均，未提供完整会话指标切换。
- min/max 为排除首尾不完整秒后的 1 秒帧计数桶极值；可包含零帧完整秒。
- 1% / 0.1% low 为最慢 `ceil(n × 比例)` 个有效帧间隔的平均倒数，至少 100 / 1000 样本才显示。
- 历史图展示最近 120 帧，隔帧取点、自动量程；横轴不是固定秒数。完整产品应升级为有时间标签的历史图。
- 采集统计按帧事件更新，快照约每250ms发布；技术验证HUD约100ms、当前桌面HUD约150ms检查状态，采样不依赖HUD重绘。
- 原型在有界会话内保留帧间隔，每链最多 200 万条，超限拒收计入 rejected；尚不适用于极高帧率长时录制或无限会话，未做最终性能验收。

## 检查

```powershell
cargo fmt --all -- --check
cargo test --locked --features hud --all-targets -j 1
cargo clippy --locked --features hud --all-targets -- -D warnings

# 会创建受控测试窗口、短暂移动鼠标并发送全局快捷键；不要在交互工作中并行运行。
# 脚本恢复原光标位置，截图仅覆盖测试窗口区域。
.\scripts\Test-Overlay.ps1
```

[目标与设置验证](docs/validation/targets-settings.md) · [初始技术验证](docs/validation/technical-spike.md) · [文档索引](docs/README.md) · [第三方许可](THIRD_PARTY_NOTICES.md)

项目仓库：[Nuxoul/NxMonitor](https://github.com/Nuxoul/NxMonitor)，默认分支 `main`。项目自身开源许可证尚待选择，仍需完整依赖/资源许可审计。跨机开发及不应上传的文件见 [开发指南](docs/DEVELOPMENT.md)。
