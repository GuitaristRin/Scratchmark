# C ABI 接口索引

权威定义在 [`include/scratchmark.h`](../include/scratchmark.h)，语义细节在各 `spec/M<n>.md`。本文件只做**分组索引**与共性约定，
不复制规格正文——避免两处不一致。

ABI 版本 `1`，SDK `0.18.0`，共 **52 个导出**。

## 共性约定

- 错误码：`0` `SM_OK`、`1` `SM_INVALID_ARGUMENT`、`2` `SM_INVALID_HANDLE`、`3` `SM_INVALID_STATE`、
  `4` `SM_BUFFER_TOO_SMALL`、`5` `SM_STALE_REVISION`、`6` `SM_INVALID_DATA`、`7` `SM_UNSUPPORTED_VERSION`、
  `8` `SM_LIMIT_EXCEEDED`、`9` `SM_INTERNAL`。含义见 [ARCHITECTURE.md](ARCHITECTURE.md#状态码)。
- 句柄 `SmHandle = u64`；`0` 无效。`sm_destroy` 之后再用同一句柄得到 `SM_INVALID_HANDLE`。
- 所有编辑接口都要带 `revision`；与当前不一致返回 `SM_STALE_REVISION`，不写任何东西。
- 输出到宿主缓冲区的接口分两派：
  - **截断派**（`sm_read_points` / `sm_read_axes` / `sm_read_brush_points`）：返回实际写入数，不报错。
  - **探测派**（`sm_save` / `sm_copy_strokes` / `sm_cut_strokes` / `sm_predict(_ex)` / `sm_resample_stroke`）：
    容量不足返回 `SM_BUFFER_TOO_SMALL` 并回填所需数量；`out == NULL && capacity == 0` 只查询。
- 只读派生（笔刷点、预测、重采样、边界、命中、套索）不改文档、不改 `revision`、不进快照。

## 生命周期与状态

| 导出 | 作用 | 规格 |
| --- | --- | --- |
| `sm_abi_version` | 返回 1 | M0 |
| `sm_features` | 能力位掩码（bit 0–16） | 各里程碑 |
| `sm_create` / `sm_destroy` | 建 / 毁文档句柄 | M0 |
| `sm_get_state` | `revision`、笔划数、活动点数、能否撤销重做 | M0 |
| `sm_get_page_id` / `sm_set_page_id` | 页面标识（设置会 bump revision，不占历史） | M3 |
| `sm_alloc` / `sm_free` | 在引擎模块内申请/释放缓冲区（给没有 malloc 的宿主） | M18 |

## 输入

| 导出 | 作用 | 规格 |
| --- | --- | --- |
| `sm_begin` | 开一笔（宽、颜色） | M0 |
| `sm_append` | 追加样本（32 字节/点，时间非递减） | M0 |
| `sm_append_axes` | 追加样本并带逐点轴向（倾斜/朝向） | M11 |
| `sm_finish` / `sm_cancel` | 提交 / 丢弃当前笔划 | M0 |
| `sm_undo` / `sm_redo` | 撤销 / 重做（每次一个历史条目） | M0 |
| `sm_read_points` / `sm_read_axes` | 读原始样本 / 轴向（分段） | M0 / M11 |

## 派生与查询（只读）

| 导出 | 作用 | 规格 |
| --- | --- | --- |
| `sm_get_stroke` | 单笔信息：宽、颜色、点数、包围盒 | M0 |
| `sm_read_brush_points` | 平滑 + 逐点完整笔宽（画多边形用） | M2 |
| `sm_hit_test` | 中心线命中测试 | M4 |
| `sm_lasso_hits` | 套索命中（返回索引列表） | M6 |
| `sm_document_bounds` | 内容并集边界（含笔宽）+ 笔划数 | M12 |
| `sm_brush_preset` / `sm_brush_preset_width` | 笔型 × 档位的归一化比例 / 实际线宽 | M13 |
| `sm_resample_stroke` | 按「笔宽 × 比例」等距取点，末点保留原始末点 | M14 |
| `sm_predict` / `sm_predict_ex` | 尾迹外推 / 带刷新率的有效窗 | M10 / M15 |

## 视口与坐标（纯函数）

| 导出 | 作用 | 规格 |
| --- | --- | --- |
| `sm_page_to_viewport` / `sm_viewport_to_page` | 页面 ↔ 视口 | M3 |
| `sm_width_page_to_viewport` / `sm_width_viewport_to_page` | 线宽换算 | M3 |
| `sm_bounds_page_to_viewport` | 包围盒换算 | M3 |
| `sm_page_to_viewport_ex` / `sm_viewport_to_page_ex` | 带旋转 | M9 |
| `sm_width_page_to_viewport_ex` / `sm_width_viewport_to_page_ex` | 带旋转的线宽 | M9 |
| `sm_bounds_page_to_viewport_ex` | 带旋转的包围盒 | M9 |
| `sm_viewport_ex_zoom_at` / `sm_viewport_ex_fit` | 锚点缩放 / 适应窗口 | M9 |

## 编辑

| 导出 | 作用 | 撤销条目 | 规格 |
| --- | --- | --- | --- |
| `sm_erase_stroke` | 整笔删除 | 1 | M4 |
| `sm_erase_path` | 沿折线切分（片段继承宽度/颜色/索引，切口带 `flags` bit1） | 1 | M5 / M17 |
| `sm_erase_indices` | 按索引批量删除 | 1 | M6 |
| `sm_clear` | 清空 | 1 | M6 |
| `sm_transform_strokes` | 选区仿射变换 | 1 | M7 |
| `sm_copy_strokes` | 复制选区为剪贴板字节 | 0（只读） | M7 |
| `sm_paste` | 粘贴剪贴板字节 | 1 | M7 |
| `sm_cut_strokes` | 复制 + 删除，**一个**条目 | 1 | M16 |
| `sm_translate_all` | 整份平移 | 1 | M12 |
| `sm_regularize_stroke` | 拟合直线/矩形/圆/箭头（可只预览） | 0 或 1 | M8 |

## 持久化

| 导出 | 作用 | 规格 |
| --- | --- | --- |
| `sm_save` | 导出快照（格式 1/2/3 按内容自动选） | M0 / M3 / M11 |
| `sm_load` | 导入快照（接受 1/2/3） | 同上 |

## 按里程碑查

| 里程碑 | 新增导出 |
| --- | --- |
| M0 | `sm_abi_version` `sm_features` `sm_create` `sm_destroy` `sm_begin` `sm_append` `sm_finish` `sm_cancel` `sm_undo` `sm_redo` `sm_get_state` `sm_get_stroke` `sm_read_points` `sm_save` `sm_load` |
| M2 | `sm_read_brush_points` |
| M3 | `sm_get_page_id` `sm_set_page_id` `sm_page_to_viewport` `sm_viewport_to_page` `sm_width_page_to_viewport` `sm_width_viewport_to_page` `sm_bounds_page_to_viewport` |
| M4 | `sm_hit_test` `sm_erase_stroke` |
| M5 | `sm_erase_path` |
| M6 | `sm_lasso_hits` `sm_erase_indices` `sm_clear` |
| M7 | `sm_transform_strokes` `sm_copy_strokes` `sm_paste` |
| M8 | `sm_regularize_stroke` |
| M9 | `sm_page_to_viewport_ex` `sm_viewport_to_page_ex` `sm_width_page_to_viewport_ex` `sm_width_viewport_to_page_ex` `sm_bounds_page_to_viewport_ex` `sm_viewport_ex_zoom_at` `sm_viewport_ex_fit` |
| M10 | `sm_predict` |
| M11 | `sm_append_axes` `sm_read_axes` |
| M12 | `sm_document_bounds` `sm_translate_all` |
| M13 | `sm_brush_preset` `sm_brush_preset_width` |
| M14 | `sm_resample_stroke` |
| M15 | `sm_predict_ex` |
| M16 | `sm_cut_strokes` |
| M17 | （无新增导出，扩展 `flags` 位语义） |
| M18 | `sm_alloc` `sm_free` |

合计 52 个，与 `include/scratchmark.h` 一致（可用 `tools/build-sdk.ps1` 的导出核对复证）。

## 结构体

| 结构 | 大小 | 字段 |
| --- | --- | --- |
| `SmSample` | 32 | `x` `y`（f64）、`time_us`（u64）、`pressure`（f32）、`flags`（u32，位域） |
| `SmSampleAxes` | 8 | `tilt` `orientation`（f32，弧度） |
| `SmStrokeInfo` | 48 | `width`、`min_x/min_y/max_x/max_y`、`color`、`point_count`、`active`、`reserved` |
| `SmState` | 40 | `revision`、`stroke_count`、`active_points`、`has_active`、`can_undo`、`can_redo`、`reserved` |
| `SmBrushPoint` | 24 | `x` `y` `width` |
| `SmBrushOptions` | 24 | `alpha`、`min_width_ratio`、`pressure_enabled`、`reserved` |
| `SmViewport` / `SmViewportEx` | 32 / 40 | 缩放、平移、（Ex 加旋转） |
| `SmBounds` | 32 | `min_x` `min_y` `max_x` `max_y` |
| `SmPoint` | 16 | `x` `y` |
| `SmPredictionOptions` / `...Ex` | — | 时间窗、长度上限、点数、笔刷；Ex 加 `refresh_rate_hz` |
| `SmResampleOptions` | 40 | `spacing_ratio`、`brush`、`reserved` |

各平台封装（C# / Kotlin / JS）按同一布局编组，见 `bindings/` 与 `tools/wasm-smoke.mjs`。
