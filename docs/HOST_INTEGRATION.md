# 宿主接入说明：供 PezMax-One 重构参考

状态：SDK 0.16.0 实验接入说明，2026-11-07。本文件只约束 Scratchmark 能力事实，不替 PezMax-One 裁定架构或技术栈；用户负责其初步重构。本轮未修改该仓。

## 可以先明确的边界

- 引擎无窗口、页面渲染器、工具栏、文件 I/O、账户或同步。宿主接收原生输入，执行页面 / 屏幕变换并绘制；引擎处理样本、派生笔迹、编辑状态和快照。
- 当前一个 Engine 是一份页面局部坐标的笔迹文档。M3 起引擎可携带宿主选择的 u64 页面标识（`sm_set_page_id` / `sm_get_page_id`，0 为未设置），快照会保存它并在载入时核对，防止把别的页面的快照恢复到当前页；页管理本身仍由宿主负责。
- 坐标和笔宽使用同一页面单位。缩放 / 平移可用引擎的 `SmViewport` 纯函数（`sm_page_to_viewport`、`sm_viewport_to_page`、线宽与边界缩放）完成：显示路径把页面几何变到视口，输入路径把屏幕坐标变回页面再 append，两条路径必须用同一份 SmViewport。DPI 计入 scale，旋转 / 翻转仍由宿主自理。颜色为 0xRRGGBBAA，不能直接当作 ARGB。
- Sample.TimeUs 是笔划内非递减的微秒值；Flags=1 表示真实压力可用，Pressure 在 [0,1]。没有真实压力时两字段均为 0，仍可正常书写；不要从设备名字推断能力或伪造压力。
- 宿主自行协调指针所有者与捕获；捕获丢失 / 操作取消时 cancel，确认笔迹时 finish。输入源 / 页面 / 视口切换的交互策略仍由宿主定义。

## 当前调用链

1. 固定匹配目标平台的 SDK，检查 ABI 1 和 sm_features 的 bit 0–14；加载库后不在文档存活期间卸载。Windows C# 现仅验收 .NET 9 / x64，不能据此假定 UWP / .NET Native 或其他宿主技术已支持。
2. create → begin(width, color) → append(批次)。不为凑批额外等待；活动笔划期间可以读取原始点或平滑 / 压力派生点。
3. get_state 取得 revision；同一 revision 读取 stroke metadata 和点，活动笔划的索引为 stroke_count。STALE_REVISION 时重新获取状态，不拼接不同 revision 的数据。
4. 原始 read_points 不变。新 read_brush_points 输出 X / Y / Width，Width 是完整笔宽。配合原 color 呈现中心线与逐点线宽；单点为圆盘，像素抗锯齿仍由宿主完成。
5. finish / cancel；空闲时 undo / redo。M4 起可在空闲时用 `sm_erase_stroke` 删除整笔；`sm_hit_test` 只查已提交笔划、多命中取最上层。命中容差先按 `sm_width_viewport_to_page` 换算到页面域再传入。局部擦除用 `sm_erase_path` 传入页面坐标折线与半径，引擎切分笔划；片段是新笔划，索引会变，成功后必须重新取 state。套索用 `sm_lasso_hits` 得到升序索引（引擎不保存选区，高亮由宿主画），再用 `sm_erase_indices` 一次删掉；`sm_clear` 一次清空也只占一个撤销条目。选区可用 `sm_transform_strokes` 平移 / 缩放 / 旋转 / 镜像 / 剪切（一次调用一个撤销条目）；复制粘贴用 `sm_copy_strokes` 产出格式 1 字节、`sm_paste` 粘贴回来（可带变换），宿主可把这份字节放进系统剪贴板或跨页使用。形状用手写后调 `sm_regularize_stroke` 规整（直线 / 矩形 / 圆 / 箭头），先用预览拿到残差再自己决定是否采纳；视口可用 `SmViewportEx` 带旋转（缩放 → 旋转 → 平移），缩放走 `sm_viewport_ex_zoom_at` 保证光标下的页面点不动，开页走 `sm_viewport_ex_fit`；预测用 `sm_predict` 每帧取一小段尾迹，与真实笔迹分开绘制，下一批样本到达后立即丢弃；笔的倾斜与朝向用 `sm_append_axes` 随样本写入（`tilt` 弧度 `[0, π/2]`、`orientation` 弧度 `[-π, π]`），引擎只保存与传播、不解释；画布增长由宿主负责：用 `sm_document_bounds` 取内容边界算出页面该多高，从顶部加长时用 `sm_translate_all` 把内容整体下移（一个撤销条目）；页面高度、顶部留白与滚动位置不入快照。笔宽可用 `sm_brush_preset_width` 从「笔型 + 档位」换算，也可以直接给具体宽度——预置表只是默认值。铺笔触用 `sm_resample_stroke`：给一个 `间距 = 笔宽 × 比例` 就得到等距点列与逐点笔宽，末点固定落在原始末点上。预测若按屏幕刷新节拍使用，用 `sm_predict_ex` 传入刷新率，引擎据一帧时长压缩时间窗。选区剪切用 `sm_cut_strokes` 一次完成，不要自己拼“复制 + 删除”，否则撤销一次只回退一半。
6. 空闲时 save 取得内存快照；设置了页面标识的文档写格式 2（魔数 `SCRATCH2`），未设置仍写格式 1。文件写入、原子替换、保留上一份有效文件由宿主负责。重开时创建引擎并 load——若引擎已设页面标识且快照标识非零不同，load 返回 INVALID_DATA，宿主可用它作为跨页保护。不要把 revision 或进程句柄持久化。

## 平滑与压力配置要一并保留

M2 是派生读取：同一份原始笔迹可用不同配置显示。快照只存原始样本与笔宽 / 颜色（格式 2 另存页面标识），不存 Alpha、MinWidthRatio、PressureEnabled 或 SmViewport。需要外观保持时，宿主在自己的文档元数据中保留读取配置与视口；本轮没有每笔专属笔刷样式持久化。

示例配置仅供实验，不能当设备手感默认值：

```csharp
using var engine = new Scratchmark.Engine(absoluteDllPath);
var brush = new Scratchmark.BrushOptions {
    Alpha = 0.5, MinWidthRatio = 0.25, PressureEnabled = 1
};
// append 输入后：
var state = engine.GetState();
var info = engine.GetStroke(state.Revision, strokeIndex);
var points = new Scratchmark.BrushPoint[info.PointCount];
engine.ReadBrushPoints(state.Revision, strokeIndex, 0, brush, points);
```

Alpha=1 关闭坐标平滑；PressureEnabled=0 关闭压力映射，MinWidthRatio 仍须有效。默认零初始化的 BrushOptions 是无效配置，应显式填写。分段读取可复用宿主数组；相同配置复用引擎缓存，不必每次取整页。

平滑使用因果低通，会有采样频率相关的滞后，笔尾不自动贴回原始末点；精确定义参 `spec/M2.md`。不得把离线示例图或 Rust 测试当作实际手感验收。

## 版本与平台事实

- 原导出 / ABI 1 / 快照格式 1 保持，SDK 0.2.0 新增能力位及派生读取；新绑定会提前拒绝不支持此能力的旧库。
- Windows GNU 原生动态、静态和 .NET 9 C# 有独立宿主运行证据。Linux 旧阶段已有动态路径证据，M2 的 Linux 包仍需复跑；MSVC / UWP / .NET Native 未验证。
- Android SDK、Studio / Kotlin 和 NDK 已就位；Rust Android 标准库、JNI / Kotlin 引擎调用与设备体验未验证。当前 Android 暂缓，不作为本轮主功能阻塞。
- 格式、算法、呈现后端与长期兼容均未正式冻结。宿主可先隔离引擎接入点，避免把实验布局散落到业务 / UI 代码；不需要先建立 submodule。
