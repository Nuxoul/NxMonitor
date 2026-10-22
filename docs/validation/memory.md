# DEV-01 内存监测验证

## 实现

- 系统已用/总物理内存：`GlobalMemoryStatusEx`，used=ullTotalPhys-ullAvailPhys，GiB。
- 目标进程工作集：`K32GetProcessMemoryInfo` 的 WorkingSetSize，MiB。不是私有内存、提交量或Unity mono。
- 只申请PROCESS_QUERY_LIMITED_INFORMATION，同一handle上前后校验创建时间及退出时间；RAII关闭handle，不注入/提权。
- runtime工作线程约每秒采样；HUD只读快照。系统即使无目标也能采样，FPS失败不影响内存。
- 最多60个采样尝试，失败为缺口；超过3秒数据过期。切换目标清进程历史，系统历史继续。系统查询失败不使用错误量程绘制旧曲线。
- 新增桌面HUD内存区与两条独立量程历史线；未修改配置schema，未接设备信息、音频等其他任务。

## 验证

Windows10 22H2 x64普通权限，本机：

- fmt、locked全目标测试、Clippy -D warnings、desktop构建均通过。
- 共27项测试通过（本次新增7项）。覆盖单位/范围、失败替换旧值、历史上限、过期、目标切换/PID身份变化、真实读取和退出进程不可用。
- 控制器集成测试：无FPS helper也能更新内存，A→B目标清空历史，B退出清空工作集。
- 原生设置选中Roblox Studio，HUD显示系统9.75/15.93 GiB，与CIM同期值一致；工作集HUD2010.3 MiB，对照Get-Process 2011.4 MiB，属于不同采样时刻的小幅波动。
- HUD内存区、曲线、底部快捷键可见；真实FPS仍工作（退出前994帧）。正常退出无本实例ETW遗留；测试使用临时配置，未写用户默认设置。
- [原生截图](memory-native.png) / [截图说明页](memory-native.html)。

无权限状态通过结果/历史测试验证，未针对真实受保护进程做手工权限实验；Windows11、DPI/多屏、全屏及发布开销不因本次测试而视为已验收。

## 一手来源

- https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex
- https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo
- https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters

已核实当前系统支持limited-query handle，WorkingSetSize单位为字节。采样数字本身随时间变化，不要求不同工具瞬时完全相等。

下一项：DEV-02 基础设备信息。本任务无剩余实现阻塞。
