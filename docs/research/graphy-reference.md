# Graphy 参考源码研究（仅阅读）

范围：本地 `D:\Code\graphy`；以下引用路径均相对该目录，行号为当前本地版本。本文仅区分现有源码行为与 Windows 迁移边界，不是 NxMonitor 的设计或实现说明。标记：【事实】源码直接证实；【迁移推断】由 Unity 内部数据来源推得、不是现成 Windows 功能；【待确认】本次源码不能证实。

## 项目定位与许可证

- 【事实】README 定位为嵌入 **Unity 项目**的 FPS、内存、音频图文监视器，兼有设备信息、调试、布局、快捷键和背景模式；README 声明 Unity 2022.3+ 与 Built-in/URP 支持。证据：`README.md:1,23-47`。这些是项目说明，不等于已经提供独立 Windows 桌面 HUD。
- 【事实】根目录确有 `LICENSE`，内容为 Martín Pane 2018 年版权的 MIT License；允许使用、修改、发布、再许可等，但复制或分发软件的全部或实质部分必须附原版权和许可声明，软件按“AS IS”提供、无担保。证据：`LICENSE:1-21`；README 也声明 MIT：`README.md:125-127`。字体等第三方素材不由这条源码许可结论自动覆盖；本次没有核定字体素材许可。

## FPS：采样、统计窗口、文本与曲线

- 【事实】`G_FpsMonitor` 是 Unity `MonoBehaviour`；每次自身 `Update()` 取 `Time.unscaledDeltaTime`，过滤非正、NaN、无穷值，令 `CurrentFPS = round(1 / delta)`（并限制为 `short` 范围）。这是运行 Graphy 的 **Unity 应用自身 Update 帧率**，不是读取其他 Windows 游戏呈现/提交帧率的外部计数器。证据：`Runtime/Fps/G_FpsMonitor.cs:14-18,45-72,139-149`。【迁移推断】若目标是跨进程 Windows 游戏 FPS，该采样入口不能直接沿用；本参考源码没有展示跨进程取帧入口。
- 【事实】平均值窗口固定为**最近至多 1024 个有效 Update 帧**的环形 `unscaledDeltaTime`；覆盖最老值时从累计帧时减去旧值，再加新值；`AverageFPS = round(样本数 / 累计秒数)`，即总帧时倒数而非逐帧 FPS 算术平均。窗口按帧数而非固定秒数，启动阶段按已有样本计。证据：`Runtime/Fps/G_FpsMonitor.cs:22-30,61-75,131-149`。
- 【事实】“1% low”和“0.1% low”对应 `OnePercentFPS`、`Zero1PercentFps`：每帧分别取 `max(1, round(样本数 × 0.01))` 与 `max(1, round(样本数 × 0.001))` 个样本；遍历环形数组有效部分维护最慢帧时降序，分别计算 `round(选中帧数 / 对应最慢帧时之和)`。两者共用上述至多 1024 帧窗口；不是瞬时 min/max。以满窗口为例，1% 为约 10 帧、0.1% 为约 1 帧（依 Unity `Mathf.RoundToInt`）；无专门的最小/最大 FPS 文本指标。证据：`Runtime/Fps/G_FpsMonitor.cs:36-39,74-113,135-149`；文本字段只有当前、ms、平均、1%、0.1%：`Runtime/Fps/G_FpsText.cs:24-29,73-91`。【待确认】若截图另有 min/max，不能据此认定当前这份 Graphy 源码已实现或两者与 low 同义。
- 【事实】显示的“当前 FPS”和 ms **另由文本组件聚合计算**：`G_FpsText.Update` 每帧累积 `Time.unscaledDeltaTime` 与帧数，当累计时间 `> 1/updateRate` 时显示 `round(帧数/累计时间)` 和 `累计时间/帧数×1000`（ms，一位小数），然后清零；平均与 low 文本才取监视器的值。因此显示“当前”通常不是 `CurrentFPS` 最近单帧倒数。默认序列化更新率为每秒 3 次，而文本类字段初始化值 4 会由管理器配置替换。证据：`Runtime/Fps/G_FpsText.cs:39-49,60-105,142-151`；`Runtime/GraphyManager.cs:148-150`。
- 【事实】FPS 历史图另以每次图组件 `Update()` 读取 `m_fpsMonitor.CurrentFPS` 写入长度为 `FpsGraphResolution` 的环形数组，默认 150 点；重设分辨率会重新建点。图每帧遍历当前点求最高 FPS；缩放最大值可按每次更新递减 1 且至少包含监视器平均值，按此最大值归一化时间顺序的点、平均线和 60/30 等阈值。这里的历史长度是**点/帧数量**而非固定时间，图中的峰值仅用于缩放，不是用户请求的历史 max 统计。证据：`Runtime/Fps/G_FpsGraph.cs:40-48,54-57,72-86,107-173`；`Runtime/GraphyManager.cs:140-150`。

## RAM：Unity Profiler 范围和曲线

- 【事实】每次 Unity `Update()` 查询 `UnityEngine.Profiling.Profiler.GetTotalAllocatedMemoryLong()`、`GetTotalReservedMemoryLong()`、`GetMonoUsedSizeLong()`，均除以 1,048,576 转 MiB：对应 Unity Profiler 的 allocated、reserved 和 Mono used 三项。最后一项明显绑定 Mono 托管堆；前两项也是 Unity Profiler 定义的内存，不应解释为 Windows 系统总 RAM、任意进程工作集或 GPU VRAM。证据：`Runtime/Ram/G_RamMonitor.cs:14-35`。【迁移推断】Windows 目标需另外核定对应指标语义，不能将这些数字直接当系统/外部游戏进程物理内存。
- 【事实】三条曲线每次图组件 `Update()` 同索引写入 allocated、reserved、Mono 的独立环形数组，默认 150 点、设置范围约 10–300；共享缩放分母是当前历史点中的**最高 reserved**，输出三条归一化曲线。没有固定秒数采样窗口，也没有此处的内存 min/max/avg 统计。文字每帧累计非缩放时间，超过 `1/RamTextUpdateRate` 才把三项 MiB 截为整数显示并清零；默认管理器设为 3 次/秒。证据：`Runtime/Ram/G_RamGraph.cs:41-53,59-62,136-203,269-300`；`Runtime/Ram/G_RamText.cs:36-75`；`Runtime/GraphyManager.cs:154-163`。分母为 0 时源码没有显式保护：`Runtime/Ram/G_RamGraph.cs:154-172`。【待确认】异常零值下的可视效果需要运行环境确认。

## 音频：来源、dB 和频谱

- 【事实】音频**来自 Unity `AudioListener` 的输出**，不是系统全局混音或任意进程音频；注释明确说自定义 FMOD/WWise 引擎不能直接工作。可指定 `AudioListener`；若未指定，按 NEVER / ON_SCENE_LOAD（默认）/ ALWAYS 策略尝试在 `Camera.main` 上查找，场景加载时还可重新查找。证据：`Runtime/Audio/G_AudioMonitor.cs:19-24,119-153,198-227`；`Runtime/GraphyManager.cs:72-77,167-172`。【迁移推断】迁移到独立 Windows 工具时，此 API 不提供系统回环采集或外部游戏音频输入。
- 【事实】每帧调用静态 `AudioListener.GetOutputData(Spectrum, 0)` 获取通道 0 输出波形，求 `RMS = sqrt(sum(sample²)/样本数)`，然后 `MaxDB = 20 log10(RMS/1)`，下限 -80 dB；名字 `MaxDB` **并非对频谱逐 bin 求最大**。随后在**同一 `Spectrum` 数组**调用 `AudioListener.GetSpectrumData(Spectrum, 0, FFTWindow)` 覆盖成频谱（默认 Blackman、512 个样本；管理器把样本数归整到 64–8192 内的 2 的幂）。证据：`Runtime/Audio/G_AudioMonitor.cs:28-39,74-97,160-172`；`Runtime/GraphyManager.cs:178-183,1104-1115`。
- 【事实】`SpectrumHighestValues` 是每个频率 bin 的峰值保持：超过历史值立即取新值，否则以 `old - old × Time.deltaTime × 2` 衰减并夹到 `[0,1]`；它**不是固定“最近几秒”滑窗的严格峰值**，虽然属性注释如此描述。证据：`Runtime/Audio/G_AudioMonitor.cs:50-53,99-117`。文本 `G_AudioText` 在 listener 可用时按非缩放时间门槛刷新、将 `(int)MaxDB` 再夹到 `[-80,0]`；与图表每帧更新率不同。证据：`Runtime/Audio/G_AudioText.cs:34-70`；`Runtime/GraphyManager.cs:178`。
- 【事实】频谱图并非音频**时间**历史：每次图 `Update()` 从当前频谱与峰值保持频谱分别把线性相邻 bin 按 `start=i*N/R`、`end=(i+1)*N/R` 求平均，`lin2dB` 将幅度 `20log10` 夹到 -160..0 后归一化 `(dB+160)/160`；每 3 个图点取平均、第三个设 -1 作条间间隙，叠加当前/峰值两层。默认 `AudioGraphResolution=81`，归整为 3 的倍数且不超过频谱大小和着色器容量；无频率对数分桶代码。证据：`Runtime/Audio/G_AudioGraph.cs:72-77,135-215,244-267`；`Runtime/Audio/G_AudioMonitor.cs:174-192`；`Runtime/GraphyManager.cs:176,1088-1096`。

## 配置、模块状态、位置、图表着色与刷新节奏

- 【事实】`GraphyManager` 在 Unity 序列化字段中配置默认开机启用、跨场景保活、黑色 alpha=0.3 背景、UI 缩放 0.5–2、FPS 阈值/颜色、三 RAM 颜色、音频图色以及各图点数和各文本更新率。`FULL/LIGHT` 是图 shader 的容量/材质模式，不是模块可见状态；各模块状态为 `FULL/TEXT/BASIC/BACKGROUND/OFF`，其中 BACKGROUND 保留组件但隐藏可视子对象和图（非“画背景”）。证据：`Runtime/GraphyManager.cs:40-61,100-191`；`Runtime/Fps/G_FpsManager.cs:123-187`；`Runtime/Ram/G_RamManager.cs:116-165`；`Runtime/Audio/G_AudioManager.cs:146-195`。
- 【事实】FPS 的 BASIC 还隐藏非基本文本；TEXT/BASIC 均关闭图；RAM/AUDIO 的 BASIC 与 TEXT 基本同路；Advanced 在 BACKGROUND 和 OFF 下都会 `SetActive(false)`。主管理器提供多模块组合 preset、`ToggleModes` 循环、`ToggleActive` 开关。证据：`Runtime/Fps/G_FpsManager.cs:143-186`；`Runtime/Ram/G_RamManager.cs:136-164`；`Runtime/Audio/G_AudioManager.cs:166-194`；`Runtime/Advanced/G_AdvancedData.cs:251-267`；`Runtime/GraphyManager.cs:79-96,857-934,936-983`。
- 【事实】FPS/RAM/AUDIO 共用一个图模块位置与 offset（默认右上角）；Advanced 有独立位置和 offset（默认左下角）；枚举支持四角及 FREE，角落设置改变 Unity `RectTransform` 的 anchor/pivot，offset 叠加原始位置并乘缩放；FREE 直接保留现有 RectTransform 位置，并无这里自动提供的拖拽或系统屏幕绝对坐标功能。证据：`Runtime/GraphyManager.cs:63-70,133-134,187-191,989-1017`；`Runtime/UI/G_ModuleManager.cs:41-94,129-159`；`Runtime/Advanced/G_AdvancedData.cs:163-215,323-330`。
- 【事实】快捷键只在管理器自身 `Update()` 且 Unity 应用有焦点和 hotkeys 开启时检查；默认 Ctrl+G 循环 preset、Ctrl+H 显隐，可配置键、Ctrl/Alt 修饰。旧输入走 `UnityEngine.Input.GetKeyDown/GetKey`；编译选项下新输入走 `UnityEngine.InputSystem.Keyboard.current`。这**不是 Windows 全局热键**。证据：`Runtime/GraphyManager.cs:113-131,814-829,1309-1465,1468-1527`。【迁移推断】不能以该热键路径推出系统范围快捷键能力。
- 【事实】监视器与图组件各自按 Unity `Update()` 运行，图没有文本更新率限制；FPS/RAM/AUDIO 的文字以各自 `1/rate` 时间门槛刷新（默认均 3 次/秒，实际按帧触发而非精确计时器）；Advanced 默认 `m_updateRate=1`，静态设备属性于 `Awake` 初始化，动态窗口/VR 数据以非缩放时间门槛刷新。应用重新获得焦点时管理器刷新参数。证据：`Runtime/Fps/G_FpsMonitor.cs:50-59`；`Runtime/Fps/G_FpsGraph.cs:54-57`；`Runtime/Ram/G_RamMonitor.cs:31-36`；`Runtime/Ram/G_RamGraph.cs:59-62`；`Runtime/Audio/G_AudioMonitor.cs:74-80`；`Runtime/Audio/G_AudioGraph.cs:72-78`；`Runtime/Fps/G_FpsText.cs:60-96`；`Runtime/Ram/G_RamText.cs:49-61`；`Runtime/Audio/G_AudioText.cs:47-60`；`Runtime/Advanced/G_AdvancedData.cs:55,106-156,333-390`；`Runtime/GraphyManager.cs:822-829`。
- 【事实】图用 Unity UI `Image.material` 按图层创建材质，`SetFloatArray` 每帧上传点。Standard 着色器数组 512、Mobile 128；管理器设 FULL 图点最多 300，LIGHT 最多 128，音频另受 spectrum 大小约束。片元用横向 UV 选值、纵向 UV 形成高度，按 good/caution/critical 阈值上色并画平均白线和阈值线，图两侧淡出，透明混合；RAM/AUDIO 将阈值与平均设 0。着色器分别含 URP 和内置管线 SubShader。这是 Unity UI/shader 实现，不代表 Windows 桌面透明窗口已实现。证据：`Runtime/Shader/G_GraphShader.cs:19-30,74-123`；`Shaders/GraphStandard.shader:17-52,87-88,131-188,194-220,283-357`；`Shaders/GraphMobile.shader:17-52,87-88,283-357`；`Runtime/GraphyManager.cs:1076-1115`；`Runtime/Ram/G_RamGraph.cs:269-300`；`Runtime/Audio/G_AudioGraph.cs:318-341`。【迁移推断】只能参考视觉与数据表达，不可直接沿用 Unity 材质/着色器调用链。

## 设备信息与未确认范围

- 【事实】Advanced 显示 Unity `SystemInfo` 提供的 CPU 型号/核数、系统内存 MB、图形 API、GPU 名称、显存/贴图上限/shader level、OS/deviceType；屏幕初值来自 `Screen.currentResolution`，窗口尺寸来自 `Screen.width/height`、刷新率和 `Screen.dpi`；条件编译 XR 还用 `XRSettings`/`XRDisplaySubsystem`。这些是**当前 Unity 应用及运行环境**信息，不是从外部目标游戏读取的窗口或设备信息。证据：`Runtime/Advanced/G_AdvancedData.cs:17-19,106-152,311-320,333-390`。【迁移推断】Windows 端的目标窗口/设备识别需单独核实。
- 【待确认】截图里如有“min/max FPS”、全系统音频、外部游戏帧率、透明置顶 HUD、全局热键或可拖拽自由摆放，仅凭本次所读源码不能确认 Graphy 实现这些能力；上述每项均应以其源码实际入口而非项目名称或视觉相似性判断。证据：FPS/音频数据入口 `Runtime/Fps/G_FpsMonitor.cs:50-59`、`Runtime/Audio/G_AudioMonitor.cs:74-97`；UI 与快捷键入口 `Runtime/UI/G_ModuleManager.cs:41-94`、`Runtime/GraphyManager.cs:814-829,1309-1527`。

本次读取：`README.md`、`LICENSE`、`Runtime/GraphyManager.cs`、`Runtime/UI/G_ModuleManager.cs`、`Runtime/Fps/G_FpsMonitor.cs`、`G_FpsGraph.cs`、`G_FpsText.cs`、`G_FpsManager.cs`、`Runtime/Ram/G_RamMonitor.cs`、`G_RamGraph.cs`、`G_RamText.cs`、`G_RamManager.cs`、`Runtime/Audio/G_AudioMonitor.cs`、`G_AudioGraph.cs`、`G_AudioText.cs`、`G_AudioManager.cs`、`Runtime/Advanced/G_AdvancedData.cs`、`Runtime/Shader/G_GraphShader.cs`、`Shaders/GraphStandard.shader`、`Shaders/GraphMobile.shader`（各模块文件名的目录沿用该条首项）。未运行构建、未读取网站、未修改参考目录。
