# 开发交接：从这里继续

最近完成 DEV-02 基础设备信息：CPU型号/核数、GPU清单、Windows build/架构缓存，目标显示器与客户区尺寸每秒刷新。下一项DEV-03长时间采集与有界统计存储。

## 先读哪些文件

1. [DEVELOPMENT.md](DEVELOPMENT.md)：新Windows电脑安装、克隆、构建、隔离配置和GitHub同步。
2. [PRODUCT.md](PRODUCT.md) 与 [requirements.md](requirements.md)：功能口径/状态、用户确认范围。
3. [ROADMAP.md](ROADMAP.md)：全部任务、优先级、验收标准。
4. [ARCHITECTURE.md](ARCHITECTURE.md)：模块、线程、资源owner与配置schema。
5. [TESTING.md](TESTING.md)：自动检查、原生测试步骤与兼容矩阵。

## 已做完，不要重复搭建

- Rust 1.96 MSVC x64，GPUI-Kit 0.8.0，匹配GPUI快照，PresentMon 2.6.0官方EXE。
- 真实外部PID采集、CSV解析、按交换链统计、FPS/ms/avg/min/max/low基础版。
- 自动前台候选、手动身份锁定、黑名单及手动覆盖确认。
- 独立设置、语言/风格/路径等阶段配置保存恢复、坏配置保护。
- 原生HUD透明/置顶/穿透/编辑/退出；后台目标切换和正常ETW清理。
- 静态Graphy视觉方向已获用户接受，**继续GPUI-Kit，不迁移Tauri/SolidJS，不继续无授权样板微调**。

推荐入口 `nxmonitor-desktop`；早期 `nxmonitor-hud`、CLI `nxmonitor`、静态 `nxmonitor-ui-reference` 均保留，各自作用见架构说明。

## 最近实际验证，而非新电脑保证

- 格式检查、锁定依赖构建、Clippy `-D warnings`通过。
- 最新全目标测试32项通过，格式、Clippy与桌面构建通过。设备原生窗口resize/minimize/restore/close测试通过。
- Windows10 22H2 x64、非管理员token下，外部Roblox Studio真实采集。
- 自动跟随测试6276条真实帧；最后手动UI流程444条；正常退出未遗留本次helper/trace。
- UI实测黑名单排除、手动确认/取消、模式切换、语言/风格保存、重启不重锁旧PID、坏配置保留。
- 证据：[阶段报告](validation/targets-settings.md)、[机器可读结果](validation/targets-settings-results.json)。这些是上一功能阶段记录，不能当作当前电脑已验收。
- 内存实测：系统9.75/15.93 GiB；HUD目标工作集2010.3 MiB，Windows同期对照2011.4 MiB。采样时间差造成小幅波动；见 [内存验证](validation/memory.md)。
- 设备实测与Windows对照：Ryzen 5 5600、6核12线程、Radeon RX 580、2560×1440 @120Hz、目标客户区2560×1377 px。见 [设备验证](validation/device.md)。
- 当前仓库没有CI；设置完整UI自动化脚本曾放在本机临时目录，并非仓库可移植测试套件。人工回归步骤已保存。

## 下一项：DEV-03 长时间采集与有界统计

先解决会话原始样本无限增长与快照排序成本，再取消600秒限制；保持目标/交换链隔离、low口径、过期语义及退出清理。不只是删时长校验。

内存和设备信息已接入，后续沿现有快照扩展，不重新实现。

后续按路线推进：长期有界采集/统计、完整时间窗口与会话展示、系统音频、真实HUD视觉整合、外观/多屏、托盘/启动、兼容/性能/发行。当前600秒限制在解除前必须先解决统计存储成本，不是删校验即可。

## 必须保留的决定

- 首版Win10/11 x64、中英双语、自用并计划开源；项目自身许可证待用户选择。
- 普通权限与非注入优先，明确提示权限问题，不自动UAC/加组/绕过反作弊。
- presented FPS不冒充displayed FPS；HUD重绘率不作为目标FPS。
- 手动锁定PID+创建时间；切Automatic清旧锁；黑名单只自动选择，手动覆盖需确认。
- 配置原子保存与队列提交一致，坏配置不覆盖、不自动采集。
- 只清理自有唯一ETW会话，不影响其他工具。
- 样本不足/无权限/过期不能显示为正常0；静态数据不得进入实时HUD。
- GUI每窗单Root；任务/订阅有owner；不用猜测的GPUI API，不混引不匹配的GPUI类型。

## 当前限制与非阻塞事项

- 当前每会话1–600秒、每链最多200万原始样本；长时内存/排序成本未解决。
- 自动选择仅基础前台窗口规则；主交换链活动量启发式可能切换。
- 固定30秒窗口未配置化、会话仅平均进入HUD、历史图不是时间轴。
- 正式HUD尚未完整整合静态Graphy视觉；音频/托盘/启动/多屏未实现。内存及基础设备信息已接入。
- Windows11、普通/无边框真实游戏、独占全屏、反作弊、DPI/多屏和Release开销未验收。
- 原用户两张参考图片未归档到仓库；构建不需要，未来精确视觉对照需用户补提供。
- 旧参考目录 `D:\Code\graphy` 不是构建依赖；字体与许可已在assets内。
- Git已按用户授权初始化，远端为 https://github.com/Nuxoul/NxMonitor ，分支为 `main`。构建产物、helper、个人配置和trace不纳入版本管理；项目自身许可证仍待选择。

这些不阻塞下一阶段本地开发；限制不等于可以声称首版已完成。

## 可直接给新电脑开发助手的提示

> 先读AGENTS.md及本页链接的产品/路线/架构/测试指南。继续Rust+GPUI-Kit，DEV-01/02已完成，下一项DEV-03长期有界采集。保留已有目标/设置/内存/设备能力，不重复搭建或迁移框架。

## 每次交接更新

写清本次任务ID、改动、实际检查、未测边界、剩余阻塞、下一项。功能状态在PRODUCT/ROADMAP更新；历史validation记录保留，不只改本页。新技能/工具若可用，可选 implement、frontend-design、tdd、diagnosing-bugs；它们不是构建依赖，不能假设其他电脑安装相同助手插件。
