# M15 - 预测的刷新率输入任务卡

状态：Canon（本阶段边界）。日期：2026-11-07。前置：M0–M14 已合入。行为真源为先行编写的 `spec/M15.md` 与 C 头文件。
触发来源：清单最后的待做项 G3——预测需要能按屏幕刷新率限制时间窗。

## 目标与真源

M10 的预测只有“最大预测时间”（`horizon_us`）。原生侧还按**刷新率**限制一帧能补多长。
本轮加一个扩展选项：刷新率大于 0 时，有效时间窗取 `min(horizon_us, 1e6 / refresh_rate_hz)`；
刷新率为 0 表示未提供，行为与 M10 逐位一致。**M10 的签名与结构不动**（新增 `sm_predict_ex` 与 `SmPredictionOptionsEx`）。

## 本轮范围

1. `SmPredictionOptionsEx`（在 M10 选项后加 `refresh_rate_hz`）与 `sm_predict_ex`。
2. 有效时间窗计算与校验；其余预测语义与 M10 一致（速度合成、长度上限、域截断、笔宽、空预测）。
3. C ABI 新增 1 个导出与 features bit 13；C# 新增 `PredictionOptionsEx` 与 `PredictEx`；加载时核对 bit 0–13。
4. C 示例、C# 静默宿主、SDK 构建与验收脚本同步更新。
5. 便签验证器自检覆盖“刷新率为 0 时与 M10 等价”与“高刷新率下预测变短”。

## 验收

- `refresh_rate_hz = 0` 与 `sm_predict` 同参数逐点相同；240 Hz 下预测长度短于 16 ms 窗；10000 Hz 由刷新率决定。
- 非法刷新率（负 / 非有限 / 超 10000）与 M10 的全部非法组合都被拒绝且不写输出。
- 原 M0–M14 断言与共享夹具不改动，全部保持通过。
- fmt / workspace test / clippy -D warnings / release 通过；SDK 独立目录 C 动态、静态与 C# 宿主实际运行通过。
- 不做帧同步、节拍回调、按帧丢弃；不声称实机延迟改善。

## 止损与边界

不改 M10 的结构与签名，不动快照；不排帧、不回调；不修改 PezMax-One、不建 submodule、不推送。
