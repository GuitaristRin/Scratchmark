# Scratchmark M0 开发 SDK

这是无 UI 的实验性原生引擎，SDK 0.4.0 / ABI 1，快照格式 1 与 2（页面标识），能力位 bit 0 支持平滑 / 压力派生读取，bit 1 支持页面标识与视口变换，bit 2 支持命中测试与整笔擦除。不提供稳定发布兼容承诺；原始行为参 `spec/M0.md`，派生笔刷参 `spec/M2.md`，页面标识与视口参 `spec/M3.md`，命中与擦除参 `spec/M4.md`。

## 包内容

- `bin/scratchmark.dll`：Windows x64 GNU 目标的动态库。
- `lib/libscratchmark.a`：同一构建的静态库，仍需链接 Windows 系统库。
- `include/scratchmark.h`：C 接口与类型声明，使用 C 调用约定。
- `examples/smoke.c`：独立宿主示例，默认动态加载；定义 `SM_STATIC` 时直接静态链接，共用行为检查。
- `verify-sdk.ps1`：核对 SHA-256 后编译并运行两种宿主，需要已安装的 GNU 目标 C 编译器与 objdump，默认使用 gcc。
- `bindings/csharp/`、`examples/csharp/`：实验 C# 适配层源码及控制台宿主，无第三方 NuGet 包；`spec/M1-CSHARP.md` 约束托管行为。
- `spec/fixtures/simple-v1.hex`、`simple-v2-page42.hex`：来自仓库共享真源的独立快照夹具（格式 1 / 格式 2）。
- `HOST_INTEGRATION.md`：宿主职责、调用链、坐标与笔刷配置保存责任。
- `build-info.json`：版本、目标、源码提交及 dirty 标记、编译器身份。
- `SHA256SUMS`：包内原始文件的校验清单，不含清单自身与后续验证产物。

在此目录的 PowerShell 中运行：

```powershell
.\verify-sdk.ps1
# 额外运行 C# 宿主，需要已有 .NET 9 SDK；禁用外部 NuGet 源。
.\verify-sdk.ps1 -IncludeCSharp
```

会创建唯一的 `verification-*` 目录，记录实际读写的快照文件，既有文件不会被覆盖。可复制整个 SDK 目录到没有引擎源码的路径再次运行，不要求 Rust 或 Git submodule。

静态链接使用 `lib/libscratchmark.a -lkernel32 -lntdll -luserenv -lws2_32 -ldbghelp`；系统库清单来自本目标的 `cargo rustc -p scratchmark-ffi --release -- --print native-static-libs`，不可外推到其他目标。验收脚本检查静态宿主 PE 导入表，并比对两种宿主生成的快照。

Linux x86_64 GNU 使用 `tools/build-sdk.sh` 与 `tools/verify-sdk.sh`，包内动态库为 `bin/libscratchmark.so`，静态库为 `lib/libscratchmark.a`。动态与静态接入、`nm -D` 导出清单、`ldd` 无引擎依赖与快照一致性均已在 Linux 独立宿主验收；详见源码仓库 `docs/STATE_2026-11-04.md`。

## 调用顺序

1. 动态加载库，核对 `sm_abi_version()`；使用派生笔刷需 `sm_features()` bit 0，页面标识 / 视口需 bit 1。创建句柄，可 `sm_set_page_id` 绑定宿主页面键。
2. `sm_begin` → 一次或多次 `sm_append`；每次输入后均可读取活动笔迹。
3. 用 `sm_get_state` 取得 revision，以同一 revision 读取笔划元数据及分段点；若状态改变，重新取得 revision。
4. `sm_finish` 确认笔划或 `sm_cancel` 丢弃；空闲时可 undo / redo。
5. `sm_save` 查询所需容量，再写入宿主缓冲区；宿主负责写盘和文件事务。
6. 新建引擎后 `sm_load` 恢复，可以继续书写；所有句柄在结束时 destroy。

同一句柄的读取步骤之间若有其他线程修改，可能收到 STALE_REVISION，这是重试信号，不是完整快照已返回。无需多线程时，宿主顺序调用即可。

## 当前能力与限制

支持固定宽度原始折线、活动笔迹、边界、整笔历史、可编辑快照、页面标识，可配置因果平滑与压力笔宽的派生读取，页面坐标 ↔ 视口坐标的纯函数变换，以及原始中心线命中与整笔擦除。原始样本不改动；没有窗口或绘图后端，视口由宿主保存。预测、局部擦除、套索、多页管理和设备防误触未实现。

设置了页面标识的文档 `sm_save` 写格式 2（魔数 `SCRATCH2` + u64 页面标识），未设置仍写格式 1；`sm_load` 两者都接受，双方非零且不同的页面标识拒绝载入。`sm_page_to_viewport` 等变换是纯函数，`SmViewport` 与显示配置一样由宿主保存，不入快照、不影响 revision。

`sm_read_brush_points` / C# `ReadBrushPoints` 接受 Alpha、MinWidthRatio 与压力开关，输出中心线及逐点完整笔宽。无压力样本仍为固定线宽；Alpha=1 关闭平滑。配置须显式有效，快照不存读取配置，宿主需一并保留它们才能重现相同外观。当前平滑会有采样相关滞后，不自动贴回笔尾；本轮未验收实机手感。

`sm_hit_test` / C# `HitTest` 只在已提交笔划中按原始中心线查找，多命中取最上层，容差为页面单位；活动笔划不参与。`sm_erase_stroke` / C# `EraseStroke` 只在空闲时按索引删除整笔，revision 不匹配返回 `STALE_REVISION`，撤销恢复原 z 序。局部擦除、套索和选择尚未实现。

Windows 包验证 Windows x64 GNU 原生 C 动态调用与静态链接，以及 .NET 9 C# 调用。C# 用法为 `using var engine = new Scratchmark.Engine(absoluteDllPath)`；每个文档协调调用与 Dispose，错误状态由 `ScratchmarkException.Status` 保留。`STALE_REVISION` 由调用方重新获取状态；文件事务仍由宿主负责。

MSVC、UWP / .NET Native 与 Android JNI / AAR 尚未验收。SO 必须按目标系统和 CPU 另行构建，不可通过改扩展名获得。C# 的 INTERNAL 停用 / 销毁策略及 finalizer 兜底已有代码，但未通过故障注入或强制 GC 验证；正常调用与显式释放已实测。

实际呈现由前端完成；此包不能证明手感或端到端延迟。未进行性能基准测试，不承诺每秒点数或调用开销比例。授权与发布策略仍待确定，本次交付是本地开发验收包。
