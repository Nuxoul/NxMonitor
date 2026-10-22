# NxMonitor 当前架构与开发约束

这是现有代码地图，不是已经实现所有首版功能的目标架构。未来新增模块按真实生命周期/能力拆分，不为每个helper建crate。功能语义见 [PRODUCT.md](PRODUCT.md)，任务见 [ROADMAP.md](ROADMAP.md)。

## 1. 包和入口

单一Cargo package `nxmonitor`，edition2024，`publish=false`。默认无GUI feature；`hud`启用GPUI-Kit、Windows接口、raw-window-handle及smol。

| 入口 | 源码 | 角色 |
| --- | --- | --- |
| `nxmonitor-desktop` | `src/bin/desktop.rs` | 当前推荐入口，创建共享model、HUD和独立设置 |
| `nxmonitor` | `src/main.rs` | 核心CLI，指定PID/采集器验证 |
| `nxmonitor-hud` | `src/bin/hud.rs` | 早期单PID HUD技术验证入口，不是桌面设置的owner |
| `nxmonitor-ui-reference` | `src/bin/ui_reference.rs` | 独立静态Canvas视觉样板，无实时采集 |

`src/lib.rs`导出capture/config/options/presentmon/stats/target。桌面模块当前通过bin中的 `#[path]` 引用，尚未统一成独立desktop crate。新增功能先沿现有边界接入，不做无关大重构。

## 2. 数据流

```text
Windows可见窗口/前台/只读进程身份
                    │
                    ▼
       desktop/runtime 工作线程
       Selection + 黑名单 + 防抖
                    │ 选择/切换/重置
                    ▼
        Capture (独立PresentMon进程)
           ├─ stdout读取线程 ─ Schema ─ Frame ─ Statistics
           └─ stderr读取线程 ─ 有界诊断文本
                    │
                    ▼
            CaptureState快照
                    │
                    ▼
    Arc<Mutex<RuntimeSnapshot>> ──▶ GPUI HUD / 设置
                    ▲
                    │ 已保存配置/运行命令
    设置回调 ─ DesktopModel ─ Runtime::commit
                    ├─ 预留队列容量
                    ├─ ConfigStore原子替换
                    └─ 发布Apply/Lock命令
```

- UI展示可用的快照，不生成目标帧，不把UI刷新率当FPS。
- 切目标时runtime后台销毁旧Capture、清空旧快照，再创建新实例；旧读取线程由旧Capture拥有。
- 目标候选和显示主交换链是两个概念：前者决定PID，后者从该PID多个链中选当前展示链。

## 3. 文件职责

| 文件 | 当前职责 | 不应塞入的工作 |
| --- | --- | --- |
| `src/target.rs` | ProcessIdentity、BlacklistRule、Selection纯规则 | Win32查询、UI对话框、配置落盘 |
| `src/config.rs` | Settings v1校验、加载、BOM兼容、同目录原子保存 | 运行时PID、GPUI Entity/窗口状态 |
| `src/presentmon.rs` | CSV按表头解析、单位选择、有效帧转换 | 子进程启动、UI文字 |
| `src/stats.rs` | 交换链隔离、滚动/会话Summary、low与桶极值 | Window/系统采样、无限会话的虚假承诺 |
| `src/capture.rs` | 单目标helper生命周期、管道读取、快照、停止唯一会话 | 自动前台选择、设置控件 |
| `src/options.rs` | 旧CLI/HUD验证参数 | 桌面持久配置owner |
| `src/desktop/processes.rs` | Win32只读进程/窗口发现与创建时间 | 统计与产品设置 |
| `src/desktop/memory.rs` | 系统物理内存/目标工作集Win32查询、身份校验、60点历史 | render查询、Unity mono分类、私有字节混称 |
| `src/desktop/device.rs` | 启动时CPU/OS/DXGI清单缓存、每秒目标窗口/显示器物理信息 | 目标图形API猜测、render系统查询 |
| `src/desktop/runtime.rs` | 后台目标控制器、命令队列、切换、发布共享快照 | GPUI渲染、业务表单 |
| `src/desktop/model.rs` | GPUI共享model、配置/命令提交、设置窗口入口 | 阻塞采集、频繁系统查询 |
| `src/desktop/settings.rs` | 持久InputState、标准设置控件、确认/反馈 | render中启动任务/保存/枚举进程 |
| `src/desktop/hud.rs` | 当前桌面HUD真实数值、图表、编辑入口 | 假内存/音频数据、另建第二份目标真相 |
| `src/native.rs` | 自有HWND样式、全局编辑/退出热键及线程清理 | 修改目标游戏窗口、注入、权限绕过 |
| `tests/config_target.rs` | 目标选择及配置存储公开边界集成测试 | 调用内部实现验证私有字段布局 |

## 4. 状态与资源所有权

### UI / model

- `DesktopModel`由GPUI Entity持有，保存当前Settings、ConfigStore、Runtime、设置窗口handle和启动错误。
- HUD和设置引用同一model。设置窗口关闭可再次打开，不为每次render重建InputState。
- 重复任务handle由对应View拥有，使用弱Entity更新，关闭View应取消任务；订阅handle保存在owner。
- 每个窗口经 `gpui_kit::open_window`只有一层Root。初始化 `gpui_kit::init`在构造组件之前执行。
- 透明HUD使用Root公开Styled覆盖默认不透明背景，不改第三方组件源码。

### runtime

- Runtime拥有一个工作线程、stop标志、最多16项的命令队列及共享快照。
- `commit`在队列锁内检查worker/stop/容量，预留后执行保存，成功才入队；不要退回“先写配置再try_send”。
- 工作线程取出命令后释放队列锁，慢速ETW清理不阻塞队列锁；UI不会执行Capture析构。
- 自动候选约650ms防抖，窗口列表约750ms刷新，控制循环约100ms；这是当前实现节奏，不是性能目标。
- runtime后台约1秒调用memory采样，结果带采样时间与PID/创建时间；切换发布时先清旧进程历史。超过3秒HUD隐藏实时内存值，失败点画缺口。内存不依赖FPS新帧或helper成功。
- device库存使用Arc缓存，动态结果带目标身份和时间；切目标清空旧值，线程DPI上下文以RAII恢复。详情在设置、HUD仅摘要。
- 打开自身设置保留有效自动目标；切Automatic清除旧手动锁定。手动模式重新恢复旧锁定会绕过黑名单确认，禁止重新引入。

### capture / native

- Capture拥有helper进程、stdout/stderr线程、共享CaptureState和唯一 `NxMonitor-<本进程PID>-<时间>` 会话名。
- helper参数显式PID、v1 metrics、stdout、timed terminate；PID0/自身不可选，避免意外全局采集。
- 解析与统计在后台，快照约250ms更新；stderr诊断文本有大小限制。
- 正常切换/退出停止唯一会话，等待helper结束，必要时终止helper并再次清理自己的会话，然后join读取线程。
- 不停止默认 `PresentMon` 会话，不调用全局trace停止，不假设强杀/断电也等同于正常析构。
- native HWND只在拥有它的GPUI线程使用；热键在自有消息线程注册/处理，Drop取消并join。
- `WS_EX_LAYERED + WS_EX_TRANSPARENT`已在Win10受控点击测试验证；单独去掉layered曾导致跨进程穿透失败。样式位、像素可见性、实际命中应共同验证。

## 5. 核心不变量

1. 进程身份是PID+启动时间，不能只存PID；重启配置没有运行时锁定。
2. 黑名单只阻止自动选择；手动覆盖必须显式确认，切换模式不得绕过。
3. UI设置生效/落盘一致；坏配置不覆盖、不可解析时安全停止自动选择。
4. 目标/交换链/会话不混算；旧目标结果不能覆盖新目标快照。
5. render只读取、派生展示和组合元素，不启动子进程、保存文件、枚举系统或无条件notify。
6. 当前读数基于真实外部事件；静态样板数据仅限独立静态入口。
7. unknown/NA/无权限/无设备/过期不等于正常0值；有意义的真实0（如完整秒内无帧或静音）需单独建模。
8. 所有循环、消息、历史与资源有owner和边界，退出只清理自己创建的资源。
9. GPUI依赖通过 `gpui_kit` facade，固定匹配快照，不混用其他GPUI版本/后端的公开类型。

## 6. 配置 schema v1

位置与默认值以 `Settings::default`为准：

```json
{
  "version": 1,
  "mode": "Automatic",
  "manual_path": null,
  "blacklist": [],
  "english": false,
  "graphy": false,
  "presentmon_path": "tools\\PresentMon-2.6.0-x64.exe"
}
```

- 枚举字符串为 `Automatic` / `Manual`；规则序列化形式为 `{"FileName":"game.exe"}` 或 `{"FullPath":"C:\\Games\\game.exe"}`。
- `manual_path`仅提示，必须绝对Windows路径；不存pid/started_at。
- version未知、空helper路径、超限/无效规则会明确失败；当前不含未来字段迁移机制。
- 当前helper默认相对路径按开发项目根/应用所在目录解析；自定义绝对路径为单机设置，不能写死到源码。
- 扩展字段时需兼容已有v1文件或显式迁移并测试，不能直接改版本让用户配置失效。
- 文档里的 [安全开发示例](../examples/settings.example.json)故意用Manual无目标，而非改变产品默认值。

## 7. 当前技术债与演进边界

- 有界600秒采集、每链200万原始样本、全会话排序/扫描：先实现有界长期聚合再解除限制。
- 当前快照锁内统计计算及重复字符串/进程列表复制是原型取舍；优化应测量后进行，避免先缓存一切。
- `stats.rs`完整秒桶有当前600秒约束，扩展时不能只改时长校验。
- main swapchain只按活动量启发式；多窗口目标可能显示链变化。
- 实时HUD和静态样板绘制分离、部分旧入口重复；整合视觉时再提取共享模块，不因本轮文档工作重构。
- 内存已由独立memory模块采样后进入RuntimeSnapshot。后续设备/音频沿窄平台边界扩展，不把系统查询塞入render。
- Windows11、多屏DPI、全屏、反作弊、Release开销未验收；不从框架API推定已支持。

## 8. 改动时应同步维护

字段/语义改变：PRODUCT、配置示例、回归测试；模块owner改变：本文件；任务完成：ROADMAP/STATUS；实测能力改变：TESTING和对应validation记录。历史研究报告保持历史属性，不反向改成新功能已经存在的证明。
