# M18 - 宿主内存分配任务卡

状态：Canon（本阶段边界）。日期：2026-11-07。行为真源为先行编写的 `spec/M18.md` 与 C 头文件。
触发原因：做 WebAssembly 封装时发现宿主没有可用的分配器，`sm_save` / `sm_copy_strokes` 这类"输出到宿主缓冲区"的接口
在 wasm 上无法调用；Android/Linux 的 C 宿主不受影响（有 libc），但同一套头文件里补一对分配接口对它们也无害。

## 本轮范围

1. `sm_alloc` / `sm_free`：8 字节对齐，`(0, 64 MiB]` 上限，失败返回 `NULL`，不进入 `guard()`。
2. features bit 16；头文件声明与注释。
3. C 侧 5 项测试（对齐、越界、释放容错、用分配出的缓冲区跑 `sm_save`）。
4. WASM 冒烟用它申请缓冲区，证明这对接口在"宿主无 malloc"的环境里确实可用。

## 验收

- fmt / workspace test / clippy -D warnings / release 通过；SDK 源码树外 C 与 C# 冒烟仍全通过。
- WASM 冒烟 53 项全通过。
- 不实现重分配、对齐参数、零初始化、内存池。

## 止损与边界

不改既有接口语义；不让引擎持有宿主内存；不修改 PezMax-One、不建 submodule；本轮不推送，等指示。
