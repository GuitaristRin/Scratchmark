# M10 - 预测笔迹任务卡

状态：Canon（本阶段边界）。日期：2026-11-07。前置：M0–M9 已合入。行为真源为先行编写的 `spec/M10.md` 与 C 头文件。
触发来源：D9 序列第五项；REFERENCE_FINDINGS 里 `原生库` 的 forecast 符号说明 参考实现 侧有笔迹预测，Scratchmark 目前只能等真实样本。

## 目标与真源

给宿主一段**只读、确定性**的预测尾迹：根据活动笔划已收到的样本外推一小段，让下一批真实样本到达前屏幕不空。
预测不是文档内容——不写文档、不进快照、不改 revision，也不参与命中与编辑。

## 本轮范围

1. `SmPredictionOptions { horizon_us, max_distance, count, reserved, brush }`：复用 M2 笔刷配置，保证预测与已绘制部分笔宽一致。
2. `sm_predict(handle, revision, options, out_points, capacity, out_written)`：
   - 输出 `SmBrushPoint` 升序数组，容量语义与 `sm_save` 一致；
   - 没有活动笔划 / 少于 2 点 / 时间跨度不为正时返回 `OK` 且零预测点；
   - 越出页面域时截断而不是报错。
3. 模型：尾段速度（最后至多 8 点）与最近两点速度各占一半，按 `horizon_us` 外推，超过 `max_distance` 时等比收缩；笔宽取最后一个笔刷点宽度并保持。
4. C ABI 新增 1 个导出与 features bit 8；C# 新增 `PredictionOptions` 与 `Predict`；加载时核对 bit 0–8。
5. C 示例、C# 静默宿主、SDK 构建与验收脚本同步更新。
6. 便签验证器在书写过程中把预测尾迹画成半透明细线，并在 `--selftest` 里覆盖预测的确定性与边界。

## 验收

- 匀速直线、拐弯（合成方向）、长度上限、无笔划 / 单点 / 零时间跨度、压力笔宽一致、越界截断、非法选项、容量不足、查询模式、`STALE_REVISION`。
- 预测不改变 revision / 文档 / 历史 / 快照；不调用预测时行为与 M9 完全一致。
- 原 M0–M9 测试断言与共享夹具不改动，全部保持通过。
- fmt / workspace test / clippy -D warnings / release 通过；SDK 独立目录 C 动态、静态与 C# 宿主实际运行通过。
- 不声称实机延迟改善、预测命中率或手感已达标。

## 止损与边界

不做机器学习模型、输入延迟补偿策略、预测与真实笔迹的融合渲染、历史点批处理；不修改 PezMax-One、不建 submodule、不推送或发布。预测样式、丢弃时机与是否启用完全由宿主决定。
