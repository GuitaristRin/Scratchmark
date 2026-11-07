# M13 - 笔型尺寸预置任务卡

状态：Canon（本阶段边界）。日期：2026-11-07。前置：M0–M12 已合入。行为真源为先行编写的 `spec/M13.md` 与 C 头文件。
触发来源：`docs/research/2026-11-07-oppo-capability-inventory.md` 的缺口 A1 / A2（笔型 × 尺寸档位）。

## 目标与真源

参考实现 有“笔型 × 5 档尺寸”的映射（`外部实现`，`Paint.java:154-223`）。Scratchmark 只让宿主直接给线宽，
没有档位概念。本轮补一个**只读预置表**：`(笔型, 档位) → 归一化比例`，以及乘最大宽度得到实际线宽的换算。

**不照抄外部行为观察里的调参值**：参考值记在清单里，我们用自己的分档（单调、有测试），理由按工作台 §6「既有基线结论不能当既定方案」。
宿主可以完全无视这张表。

## 本轮范围

1. `sm_brush_preset(kind, level, out_ratio)`：五个笔型 × 五个档位的归一化比例，纯函数。
2. `sm_brush_preset_width(kind, level, max_width, out_width)`：`max_width × ratio`，值域校验。
3. C ABI 新增 2 个导出与 features bit 11；C# 新增 `BrushKind`、`BrushPresets.Ratio/Width`；加载时核对 bit 0–11。
4. C 示例、C# 静默宿主、SDK 构建与验收脚本同步更新。
5. 便签验证器把宽度输入改成“笔型 + 档位”选择（内部仍落到具体线宽），并在自检里覆盖预置表。

## 验收

- 25 个组合全部可查；比例在 `(0, 1]` 且同一笔型内严格递增。
- 宽度换算在 `max_width = 1 / 10 / 1e6` 下与手算一致。
- 非法 `kind` / `level` / `max_width` 一律拒绝且不写输出。
- 原 M0–M12 断言与共享夹具不改动，全部保持通过。
- fmt / workspace test / clippy -D warnings / release 通过；SDK 独立目录 C 动态、静态与 C# 宿主实际运行通过。
- 不声称这些默认值在真机上“好看”，也不做纹理与透明度档位。

## 止损与边界

不做笔触纹理、透明度档位、按笔型的平滑差异、倾斜映射；预置表不进快照、不影响几何；不修改 PezMax-One、不建 submodule、不推送。
