# 换电脑开发与 GitHub 同步指南

本项目是Windows原生应用，不需要Node、pnpm、Tauri或SolidJS。新电脑应从仓库源码重建，不要复制旧电脑的 `target/`。所有命令按Windows PowerShell书写，在仓库根目录执行。

## 1. 新电脑准备

### 必需软件

1. Git for Windows。
2. Visual Studio 2022 Build Tools或Community，安装 **Desktop development with C++**，包含MSVC x64工具与Windows SDK。
3. CMake，确保构建终端的PATH中可执行。
4. Rust MSVC x64工具链。本机通过验证的版本是 **1.96.0**，`Cargo.toml`要求至少1.96、edition2024；仓库没有 `rust-toolchain.toml`，因此请明确检查实际版本。

官方入口：

- https://git-scm.com/downloads/win
- https://visualstudio.microsoft.com/downloads/
- https://cmake.org/download/
- https://rust-lang.org/tools/install/

在另一台Windows机器是否安装软件、提权安装C++工具由机器所有者授权；不要由开发助手悄悄改系统用户组或安全设置。

### 检查工具链

```powershell
git --version
cmake --version
rustup show active-toolchain
rustc --version
cargo --version
```

Rust应为 `x86_64-pc-windows-msvc`，不是GNU或ARM64。如需重现当前基线，可在已安装rustup后执行：

```powershell
rustup toolchain install 1.96.0-x86_64-pc-windows-msvc --profile minimal --component rustfmt --component clippy
# 下面命令用 +版本，避免更改整台机器默认工具链。
cargo +1.96.0-x86_64-pc-windows-msvc --version
```

后续命令中的 `cargo` 可替换为上面的带版本形式。若找不到已安装Rust，先打开新终端，或仅修正当前进程PATH：

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

缺 `link.exe`、Windows SDK或C++库时，在Visual Studio Installer核对workload；必要时使用 **Developer PowerShell for VS 2022**。不要通过随意切GNU目标或替换GPUI版本绕过缺失组件。

## 2. 从 GitHub 取得源码

仓库地址如下；默认开发分支为 `main`。

```powershell
$repoUrl = 'https://github.com/Nuxoul/NxMonitor.git'
git clone $repoUrl
Set-Location .\NxMonitor
git status --short
```

已有克隆的电脑先检查工作树：

```powershell
git status --short
git branch --show-current
# 仅在工作树已妥善处理且确认分支后同步；--ff-only防止意外创建合并提交。
git pull --ff-only
```

不要用 `reset --hard`、`clean -fd` 清掉另一台电脑未提交的修改。多电脑同时开发请使用独立分支、短提交/PR；不要同时改同一工作树。

## 3. 构建并取得采集器

保留仓库中的 `Cargo.lock`，优先 `--locked`；无必要不要先 `cargo update`。

```powershell
cargo fetch --locked
cargo build --locked --features hud --bin nxmonitor-desktop -j 1

# 显式下载官方PresentMon 2.6.0 x64；脚本核对固定SHA-256与Intel签名。
.\scripts\Get-PresentMon.ps1
```

- `hud` 是可选Cargo feature，**不是默认feature**。不启用时只构建核心CLI/库，不能得到桌面程序。
- 首次依赖下载/编译可能耗时数分钟，需要crates.io、GitHub及相关构建资源网络访问。
- helper位于 `tools/PresentMon-2.6.0-x64.exe`，被Git忽略，需要每台机器重新获取；不要上传或从不明渠道拷贝替代文件。
- `Get-PresentMon.ps1` 是开发获取脚本，不是安装器，不安装服务、不自动提权。
- 如果PowerShell执行策略阻止脚本，先查看 `Get-ExecutionPolicy -List`，按本机/组织政策审核后运行。不要全局关闭安全策略；只有经授权时才考虑当前进程范围的设置，不能绕过组织策略。

首次可用性检查：

```powershell
.\target\debug\nxmonitor-desktop.exe --help
.\target\debug\nxmonitor-desktop.exe --list-processes
```

## 4. 使用隔离配置开发

建议给每台机器、每个测试保留独立配置，不把私人绝对路径提交到仓库。

```powershell
New-Item -ItemType Directory -Force .\local | Out-Null
Copy-Item .\examples\settings.example.json .\local\settings.json
.\target\debug\nxmonitor-desktop.exe --config .\local\settings.json
```

`local/`被Git忽略。示例默认手动模式、无目标，便于安全启动。启动后在设置中选择进程，黑名单、语言和预设会保存到该文件。

正式默认位置是 `%LOCALAPPDATA%\NxMonitor\settings.json`，**不在仓库内，不会随Git克隆同步**。确实要迁移偏好可手工复制，但新电脑应重新选择采集器路径并核对黑名单路径；PID从不迁移。不要公开上传此文件，其中可能有个人安装路径。

当前schema示例见 [settings.example.json](../examples/settings.example.json)；改schema前读 `src/config.rs`，增加迁移/兼容测试，不静默丢掉旧黑名单。

默认helper相对路径按开发项目根/应用目录解析；正常克隆不需要复刻旧电脑的 `D:\Code\NxMonitor`。启动入口会把相对 `--config` 解析为当前工作目录下的绝对路径。

## 5. 程序入口

| 二进制 | 用途 | 命令 |
| --- | --- | --- |
| `nxmonitor-desktop` | 推荐继续开发入口，HUD+独立设置 | `cargo run --locked --features hud --bin nxmonitor-desktop -- --config .\local\settings.json` |
| `nxmonitor-hud` | 保留的单PID技术验证HUD | `cargo run --locked --features hud --bin nxmonitor-hud -- --help` |
| `nxmonitor` | 无图形核心采集CLI | `cargo run --locked --bin nxmonitor -- --help` |
| `nxmonitor-ui-reference` | 无采集静态视觉样板 | `cargo run --locked --features hud --bin nxmonitor-ui-reference` |

- HUD默认鼠标穿透；编辑 `Ctrl+Alt+F6`，退出 `Ctrl+Alt+F7`；冲突时按界面显示的备用组，不假设全机器可注册同一热键。
- 设置窗口的关闭≠整个应用退出。用“退出 NxMonitor”或全局退出结束并等待清理。
- 单会话当前最多600秒，设置中可重置。不要把原型当最终长期监测程序。
- 发布构建命令为 `cargo build --locked --release --features hud --bin nxmonitor-desktop -j 1`；命令可用不代表已经完成发布版性能或干净机器验收。

## 6. 开始改代码前

按顺序读：

1. [STATUS.md](STATUS.md)：目前状态与下一项。
2. [PRODUCT.md](PRODUCT.md) / [requirements.md](requirements.md)：产品口径与已确认需求。
3. [ROADMAP.md](ROADMAP.md)：只选一个有边界的任务。
4. [ARCHITECTURE.md](ARCHITECTURE.md)：状态和资源owner。
5. [TESTING.md](TESTING.md)：检查与兼容性证据。
6. 相关源码、测试、组件官方文档；编辑UI前读本地GPUI设计/编码指南。

DEV-01内存与DEV-02设备信息已完成，下一项DEV-03长期有界采集。不要重复搭建、迁移UI框架或把静态示例数据当实时读数。

## 7. 每次提交前的最低检查

```powershell
cargo fmt --all -- --check
cargo test --locked --features hud --all-targets -j 1
cargo clippy --locked --features hud --all-targets -- -D warnings
cargo build --locked --features hud --bin nxmonitor-desktop -j 1
```

桌面/权限/ETW/音频有相关改动时，还需做 [TESTING.md](TESTING.md) 对应实测。没有相应OS/硬件时写“未测”，不能把单元测试或静态截图当平台验证。

最新功能阶段32项测试通过；数量随功能演进，不作为固定配额。

## 8. 本地环境常见问题

- **cargo stderr被PowerShell当错误**：Cargo进度也写stderr，依据退出码和首个真正错误判断；`$ErrorActionPreference='Stop'`可误中断原生命令输出。可以重定向日志，随后显式检查 `$LASTEXITCODE`。
- **依赖metadata/rlib缺失、can't find crate、std stub级联错误**：先确认没有另一个cargo在同一target目录运行，用 `-j 1` 重试；查看首个错误。仅确定缓存问题时定向清理相关包并重建，不反复删全仓库/源码。
- **窗口空白、截图裁切**：区分真实绘制、窗口client bounds和截图物理坐标；先核对DPI/客户区，截图不是业务正确性的充分证据。静态样板过去出现过此类问题。
- **半透明没有效果**：GPUI Component Root默认有不透明背景；现有HUD已通过公开Root Styled覆盖。不要重新叠不透明背景，也不要随意删除已实测必需的Windows穿透样式。
- **权限不足/无帧**：使用自己有权监测的图形应用、核对实际PID。普通账户的ETW组成员条件可能不同，unknown名称警告不等于采集失败；不自动提权/加组。看诊断，不填假数据。
- **UI自动化按钮无效果**：GPUI无障碍树可能将被滚动裁切的控件标为可见；核对实际bounds、滚到可见范围，用正确的Invoke/Toggle/SelectionItem/Value模式，操作后重新读树。
- **坏配置**：备份后修复或重命名原文件并重启。应用会拒绝静默覆盖；不要在测试时破坏用户默认配置。
- **程序被强制结束后有ETW会话**：正常退出与崩溃清理不是同一保证。只处理已核实属于该次实例的 `NxMonitor-<PID>-<时间>` 名称，命令见 [TESTING.md](TESTING.md)；不能停止默认PresentMon或其他应用会话。

## 9. GitHub 首次上传与跨机同步清单

### 应保留在仓库

`Cargo.toml`、**Cargo.lock**、`src/`、`tests/`、`scripts/`、`examples/`、`assets/fonts/`和其许可、`docs/`、`README.md`、`AGENTS.md`、`THIRD_PARTY_NOTICES.md`、`.gitignore`。

### 不要上传

- `target/`、构建EXE/PDB、helper下载文件。
- `local/`、`%LOCALAPPDATA%`个人配置、日志、CSV/ETL trace、账户/设备/私人路径数据。
- 本次开发助手的scratch临时目录与 `.learnings/`。**仓库文档不依赖scratch存在**。
- Token、SSH密钥、GitHub凭据、`.env`秘密；当前工程不需要这些文件才能构建。

现有 `.gitignore`是防护而不是审计。上传前用户或获授权代理应执行 `git status --short`、检查实际暂存列表和diff，尤其是截图/JSON/日志里的私人路径；不要直接无脑 `git add .`。即使设置成私有仓库，也不应提交凭据。

项目已按用户授权初始化Git，`origin`绑定 `https://github.com/Nuxoul/NxMonitor.git`，默认分支为 `main`。源码、锁文件、交接文档与第三方许可说明纳入版本管理，本机产物被忽略。项目自身开源许可证仍待用户选择，不因上传源码而自动授予开源许可。GitHub Issues可按路线中的任务ID逐项建立；CI仍属于待做 `REL-01`。

每次换电脑前提交/推送自己的任务分支；另一台机器拉取后先运行基线检查，再开始下一任务。遇到未提交用户改动先保留，不用破坏性命令“恢复干净”。

## 10. 本机参考目录不是构建依赖

- 原参考源码路径 `D:\Code\graphy` 只是本机阅读来源，新电脑不必存在。核心研究已保存至 `docs/research/graphy-reference.md`。
- Roboto字体及许可已在 `assets/fonts/`，构建不需要原Graphy目录。
- 原两张用户截图仍在聊天附件，**未作为原始参考图归档到仓库**。已有静态样板源码/本机截图不等于原图。如未来要逐像素对照，用户需另行提供可归档原图及分发许可；不阻塞当前功能开发。
- GPUI指南Markdown可离线读，图片仍是远程链接；不是完整离线网站镜像。
