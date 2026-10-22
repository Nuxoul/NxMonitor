# DEV-02 基础设备信息验证

## 实现

- 启动时后台缓存CPU型号、物理核心数、全部处理器组逻辑处理器数、Windows build/release/edition及x64架构。
- DXGI枚举本机硬件适配器，显示名称、VEN/DEV和LUID，排除软件适配器。即使同名也不猜测为同一GPU；不是目标使用GPU判定。
- 目标显示器和窗口客户区每秒后台刷新，显示屏幕编号、系统报告分辨率及整数Hz。刷新率不是FPS，不报告未检测的目标图形API。
- 无目标时主屏明确标记；目标多窗口优先前台，否则最大可见客户区。最小化、无窗口、身份变化、失败和超过3秒未更新时明确不可用。
- 线程临时使用per-monitor-v2 DPI上下文读取物理像素，RAII恢复；不更改系统缩放或目标窗口。
- 设置窗口显示完整清单，HUD只加屏幕/客户区摘要；配置schema不变。

## 检查与本机实测

Windows10 22H2 x64普通权限：

- fmt、locked全目标测试（-j 1）、Clippy -D warnings、desktop构建通过。
- 共32项测试通过，本次新增5项：变长核心拓扑校验、UTF16/矩形处理、目标变化清空并保留缓存、真实库存查询/错误身份，以及原生受控窗口320×240→640×360、最小化、恢复和关闭。
- Windows CIM对照：AMD Ryzen 5 5600，6物理核心/12逻辑处理器；Radeon RX 580 2048SP，VEN1002/DEV6FDF；Windows10 build19045，22H2，Professional，x86_64。
- DXGI在本机返回两个同名硬件条目，因此保留清单并加入LUID区分，不声称目标使用其中哪项。
- 实际显示2560×1440 @120Hz；外部Roblox Studio客户区2560×1377 px与Win32客户区直接测量一致。
- 设备详情可滚动查看，长型号与中文标题可读，HUD底部快捷键可见；真实FPS与内存仍正常工作。采集结束正常清理，未改默认配置。
- [HUD截图](device-hud.png)、[设置截图](device-settings.png)、[展示页](device-native.html)。

Windows11、多屏/混合DPI、显示器热插拔、GPU热插拔、全屏及发布版性能未因此验收。设备清单重启刷新，动态屏幕模式每秒刷新；Hz为整数报告值，不保证59.94等小数精度。

## 一手来源

- https://learn.microsoft.com/en-us/windows/win32/api/dxgi/nf-dxgi-idxgifactory1-enumadapters1
- https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getlogicalprocessorinformationex
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getclientrect
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaysettingsw
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmonitorinfow

CPU/Windows文字使用只读系统注册表（缺字段则不可用），不运行外部探测脚本。下一项DEV-03长时有界采集与统计，本项无剩余阻塞。
