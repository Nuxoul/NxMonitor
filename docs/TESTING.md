# 测试、实机验收与证据规范

本文件提供换电脑后的回归方法。最新已有阶段证据见 [STATUS.md](STATUS.md)、[targets-settings.md](validation/targets-settings.md) 与 [结果JSON](validation/targets-settings-results.json)。本机历史测试通过不等于新电脑已经通过。

## 1. 自动检查

在Windows MSVC环境、仓库根目录运行：

```powershell
cargo fmt --all -- --check
cargo test --locked --features hud --all-targets -j 1
cargo clippy --locked --features hud --all-targets -- -D warnings
cargo build --locked --features hud --bin nxmonitor-desktop -j 1
```

最新基线32项：原有27项加5项设备测试；含真实CPU/OS查询、变长核心记录校验、目标切换清空，以及受控窗口resize/minimize/restore/close。UI仍需原生实测，数量随功能演进。

当前测试位置：

- `src/presentmon.rs`：表头大小写/BOM、时间单位、NA/非法值、CSV引用字段。
- `src/stats.rs`：帧时间平均、low样本量、完整秒桶、链隔离、旧链过期/滚动窗口。
- `src/capture.rs`：参数保护、零帧完成/错误保留。
- `tests/config_target.rs`：自动/手动/黑名单/PID复用、配置替换/坏配置/BOM。
- `src/desktop/runtime.rs`测试模块：队列容量预留、保存失败不发布。
- `src/desktop/memory.rs`：二进制单位、无效值、失败缺口、60点上限、过期、目标身份/退出及真实Windows查询。runtime测试切目标与退出清空、FPS失败不影响内存。
- 最新内存实机核对见 [memory.md](validation/memory.md)，无权限真实保护进程/Windows11未另做实测。
- 设备信息实测见 [device.md](validation/device.md)；本机单屏通过，Windows11、多屏/混合DPI未因此验收。

新增测试优先覆盖公开行为边界。不要只测试实现字段或重复生产算法算一遍当期望值；平台系统调用可放窄适配层，纯单位/状态逻辑可独立测。

## 2. 不触碰用户设置的 smoke test

先复制示例到忽略目录：

```powershell
New-Item -ItemType Directory -Force .\local | Out-Null
Copy-Item .\examples\settings.example.json .\local\smoke.json
.\target\debug\nxmonitor-desktop.exe --config .\local\smoke.json --smoke-seconds 10
```

确认HUD与设置出现、无假FPS、到期退出。示例Manual无目标，不能将无帧看作失败的采集验证。`--smoke-seconds`只检查生命周期，不是完整交互测试。

## 3. 纯窗口受控回归

仓库已有 `scripts/Test-Overlay.ps1`，它测试保留的 `nxmonitor-hud` 验证入口，不是新设置的全流程脚本。

```powershell
cargo build --locked --features hud --bin nxmonitor-hud -j 1
.\scripts\Test-Overlay.ps1
```

该脚本创建已知背景窗口、取样/截图、移动鼠标、点击受控窗口、发送全局编辑/退出快捷键。只在可交互桌面、无其他人同时操作时执行，不能放进无人看管的日常工作中。恢复光标不等于毫无交互干扰。

可连接有权监测的真实外部PID：

```powershell
.\scripts\Test-Overlay.ps1 -CapturePid 12345 -PresentMon .\tools\PresentMon-2.6.0-x64.exe -Language zh -Style graphy
```

替换PID。脚本输出证据目录；将需要保留的去隐私证据复制到项目task相关validation目录，不把临时目录硬编码进永久文档。

### 已知脚本局限

- 测试像素依赖已知背景、屏幕位置和缩放，换DPI/主题可能需调整harness；先检查目标和几何，不立刻改产品代码让测试通过。
- 静态样板和早期截图曾有裁切/遮挡。截图、样式位、可见像素和实际鼠标命中共同判断，不用一个信号代替全部。
- 新设置窗口的UI Automation流程曾在本机临时脚本实测，**这些scratch脚本未作为可移植测试套件纳入仓库**。下面保留完整人工/新脚本步骤；不能声称当前有一键设置E2E或Windows CI。

## 4. 目标/设置完整回归

使用独立 `--config`，目标必须是你有权监测且确实产生Present帧的图形应用。测试不必复刻旧机器的Studio PID，所有PID实时获取。

| 步骤 | 预期结果 |
| --- | --- |
| Manual无目标启动，按名称/PID筛选 | 真实窗口应用列表可操作，读数不可用而非演示值 |
| 锁定进程A，切到应用B | 仍锁A，日志新建A会话，真实帧统计可更新 |
| 将A加入文件名黑名单 | 保存成功，现有手动会话不因添加规则重置 |
| 切Automatic，把A置前台 | 黑名单阻止A自动选择，不自动新建A会话 |
| 切回Manual但不选目标 | 不恢复旧A锁定，不绕过黑名单确认 |
| 点击锁定A，取消覆盖 | 不锁定、不改变为已确认状态 |
| 再点击并确认覆盖 | 新建手动A会话，不混旧统计 |
| 修改语言/风格 | 界面即时变化、落盘；不重置同目标统计 |
| 重置采集 | 新会话、旧快照清空 |
| 移除规则、切Automatic，把A置前台 | 稳定前台后选A |
| 激活NxMonitor设置 | 保留仍有效A，不误监测自身、不重建会话 |
| 结束目标进程/重新启动 | 原身份不复用，手动不会静默锁定新实例 |
| 正常退出 | helper结束，本次唯一ETW会话消失 |
| 重启保存配置 | 语言/风格/黑名单恢复，手动仅路径提示、不恢复PID |
| 构造损坏JSON再启动 | 错误可见、不自动采集、不覆盖原字节；测试后恢复/删除隔离文件 |
| UTF-8 BOM文件 | 正常加载，不以BOM误报损坏 |

UI Automation请每次更新后重新读控件树，以role/name/value/selection作为主要定位。GPUI当前可能把滚动裁切控件的IsOffscreen标为false；先检查bounds、滚到可见区域，再使用Invoke/Toggle/SelectionItem/Value契约。未经确认的屏幕坐标点击是最后手段，不能落入用户其他应用。

## 5. 真实FPS与清理

```powershell
.\target\debug\nxmonitor-desktop.exe --list-processes
.\target\debug\nxmonitor.exe --presentmon .\tools\PresentMon-2.6.0-x64.exe --pid 12345 --seconds 15
```

检查实际表头/口径、PID/链分组、样本数、等待和过期。渲染暂停/最小化可能不产生帧，不直接认定采集器失败。当前呈现FPS不与显示器刷新率或任务管理器数值机械等同。

正常退出前记录日志中的实例PID与唯一ETW会话，退出后检查：

```powershell
Get-Process nxmonitor-desktop, nxmonitor-hud -ErrorAction SilentlyContinue
Get-CimInstance Win32_Process |
    Where-Object { $_.Name -like 'PresentMon*' } |
    Select-Object ProcessId, ParentProcessId, CommandLine
logman query -ets | Select-String 'NxMonitor-'
```

不要终止所有PresentMon进程。测试被强杀/崩溃后，只有确认某个唯一会话属于该次测试且已无owner时，才可显式清理：

```powershell
$sessionName = 'NxMonitor-REPLACE_WITH_VERIFIED_INSTANCE_NAME'
.\tools\PresentMon-2.6.0-x64.exe --session_name $sessionName --terminate_existing_session
```

这是开发恢复操作，不是让用户日常手动清理资源的替代。正常退出路径必须自己释放；权限失败写清楚，不能自动提权/改组或停止其他trace。

## 6. 兼容性验收矩阵

| 环境 | 采集 | HUD覆盖/输入 | 当前证据 |
| --- | --- | --- | --- |
| Windows10 22H2 x64，外部Roblox Studio窗口 | 已实测成功 | 透明/置顶/穿透在受控窗测试；设置真实操作通过 | 阶段报告 |
| Windows10真实游戏普通窗口 | 待记录 | 待记录 | 不以Studio替代 |
| Windows10真实游戏无边框全屏 | 待记录 | 待记录 | 首版必补 |
| Windows11 x64窗口/无边框 | 待记录 | 待记录 | 首版必补 |
| 独占全屏 | 单独验证 | 单独验证，可记录不支持 | 不承诺普遍覆盖 |
| 普通账户不同ETW组身份 | 待扩展 | 不适用/分别记 | 本机非管理员token成功不是全账户保证 |
| 带反作弊目标 | 仅按目标允许范围测试 | 分别记，不绕过 | 未验收 |
| 多屏100%/125%/150%/200%、屏幕断开 | 待做 | 位置/缩放/穿透/焦点 | 未验收 |
| Release长时运行 | >600秒当前实现未支持 | 内存/CPU/GPU/响应 | 需DEV-03/QA-03 |

每条新记录至少包括：OS build、CPU/GPU/驱动、应用版本/图形API（已知才填）、窗口模式、权限身份、构建模式、持续时长、采集/覆盖分别结果、错误/截图/日志路径。避免提交个人路径、用户名、无关桌面或账户信息。

## 7. 功能开发后的额外验收

- 内存：单位与来源、读权限不足、目标退出/切换、历史上限。
- 音频：静音/无设备/切换默认设备、已知信号RMS与FFT、退出释放、无录音存盘。
- 外观：两种语言、长标签/CJK、键盘/焦点、最低窗口、缩放、颜色对比、错误路径。
- 托盘/启动：隐藏/关闭/退出语义、全局快捷键冲突、用户禁用启动项、普通权限启动。
- 性能：使用Release测量，记录NxMonitor自身开销与测量方法；预算未定前不宣称“低于某百分比”。

## 8. 记录原则

- 自动测试、交互实测、截图视觉检查、平台未测分开报告。
- 不覆盖历史证据制造“新机器通过”；新建记录并更新STATUS/矩阵。
- 本轮仅文档更新时不必重复运行所有桌面测试，但应检查链接、JSON示例、命令与当前源码一致，并声明没有重做实机验收。
