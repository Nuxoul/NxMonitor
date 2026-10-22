# NxMonitor 文档入口

## 新电脑 / 新开发会话先读

1. [当前进度与下一项](STATUS.md)：快速接手，不依赖旧聊天或临时文件。
2. [跨电脑开发与GitHub同步](DEVELOPMENT.md)：Windows环境、构建、运行、配置隔离和上传清单。
3. [功能说明](PRODUCT.md)：产品语义与已完成/待做范围。
4. [整体开发路线](ROADMAP.md)：任务ID、优先级、依赖、验收标准。
5. [当前架构](ARCHITECTURE.md)：源码地图、状态owner、线程/资源生命周期、schema。
6. [测试指南](TESTING.md)：自动检查、桌面回归与兼容矩阵。

根目录还有 [README](../README.md)、[AGENTS.md](../AGENTS.md) 和 [安全配置示例](../examples/settings.example.json)。当前推荐程序入口是 `nxmonitor-desktop`，GUI需启用 `hud` feature。

## 文档权威与维护顺序

- **用户确认的需求**：[requirements.md](requirements.md)。更改产品范围需要明确确认，不能将路线建议当新需求。
- **当前功能解释**：PRODUCT；**待办与验收**：ROADMAP；**本次接续点**：STATUS。
- **当前实现**以源码/测试为准；模块边界改变同步ARCHITECTURE。
- validation报告记录某次环境的证据，不证明另一台机器已通过，也不将历史研究改成当前能力。
- DEV-01内存与DEV-02设备信息已完成，下一项DEV-03长期有界采集。音频、视觉整合和桌面生命周期仍待做。

## 阶段验证记录

| 记录 | 定位 |
| --- | --- |
| [设备验证](validation/device.md) | 最新阶段：32项测试、设备/屏幕/窗口实测 |
| [内存验证](validation/memory.md) | 内存阶段：27项测试、系统/目标内存实测 |
| [目标/设置闭环报告](validation/targets-settings.md) / [结果JSON](validation/targets-settings-results.json) | 上一阶段：20项测试与真实设置/自动目标流程 |
| [初始技术验证](validation/technical-spike.md) / [结果JSON](validation/results.json) | 早期FPS/原生窗口PoC，历史证据 |
| [早期HUD截图页](validation/overlay-evidence.html) | 原生窗口截图，不是完整首版设计验收 |
| [静态Graphy样板页](validation/ui-reference.html) / [源码](../src/bin/ui_reference.rs) | 无监测样例，用户已接受方向；截图曾有尺寸/裁切问题，不作精确golden |

[第三方许可与字体说明](../THIRD_PARTY_NOTICES.md)。自身开源许可证及完整发行审计仍待处理。

## GPUI-Kit 官方指南本地副本

| 文档 | 可读正文 | 原始网页 | 校验记录 |
| --- | --- | --- | --- |
| Design Guides | [设计指南](references/gpui-kit/design-guides.md) | [HTML](references/gpui-kit/design-guides.html) | [JSON](references/gpui-kit/design-guides.verification.json) |
| Coding Guides | [编码指南](references/gpui-kit/coding-guides.md) | [HTML](references/gpui-kit/coding-guides.html) | [JSON](references/gpui-kit/coding-guides.verification.json) |

来源与署名：**GPUI Kit**。

- https://gpui-kit.com/zh-CN/docs/design-guides/
- https://gpui-kit.com/zh-CN/docs/coding-guides/

抓取日期见文件头部。Markdown保留正文/代码/表格/图片链接，移除导航、脚本、样式和高亮标签；图片仍远程，不是完整离线镜像。原始HTML未修改。许可与转存修改说明见 [第三方声明](../THIRD_PARTY_NOTICES.md#archived-documentation)；API须按锁定依赖版本核对，不能仅凭在线新文档猜接口。

## 历史研究，不是构建依赖

- [Graphy源码研究](research/graphy-reference.md)：Unity数据源、统计与shader参考，附原本地源码路径/行号。
- [第一轮理解](research/windows-fps-counter-understanding.md)：早期功能映射和当时未验证项，后续状态看STATUS/PRODUCT。

原 `D:\Code\graphy` 是阅读来源而非构建依赖，新电脑不需要同样盘符。Roboto已带许可放入assets；用户原始两张截图未作为源图归档。如后续需要精确图像对照，应请用户补提供原图，不假装已有。
