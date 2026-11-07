# 平台与封装库

本文件写清**每个平台现在能交付什么、验证到哪一步、还差什么**。判据只有一条：能不能在本机真的跑起来。
不要把它读成"支持矩阵"——没有验证过的一律标出来。

引擎接口只有一套：`include/scratchmark.h`（C ABI 1），其余平台都是它的封装。
SDK 版本见 `Cargo.toml`；能力位见 `sm_features()`（当前用到 bit 0–16）。

## 一览

| 平台 | 产物 | 构建 | 运行验证 | 状态 |
| --- | --- | --- | --- | --- |
| Windows x64 | `scratchmark.dll` / `.lib` | 本机 gnu 工具链 | ✅ C 662 + C# 547 项 | 已交付 |
| WebAssembly | `scratchmark.wasm` | 本机（wasm32-unknown-unknown） | ✅ Node 里 53 项 | 已交付 |
| Android | `jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/libscratchmark.so` | 本机 + NDK 30 | ❌ 无设备/模拟器 | 产物已生成，仅编译与符号核对 |
| Linux x86_64 | `libscratchmark.a`（静态，musl） | 本机 | ❌ 无 Linux 运行时 | 仅编译与符号核对 |
| iOS | 源码与脚本 | ❌ 需要 macOS + Xcode | ❌ | 尚未开始 |
| .NET (NuGet) | `Scratchmark.0.18.0.nupkg`（含 `runtimes/win-x64/native/scratchmark.dll`） | 本机 | ✅ 消费方装包后 11 项 | 已交付 |

## WebAssembly

```powershell
cargo build -p scratchmark-ffi --release --target wasm32-unknown-unknown
node tools/wasm-smoke.mjs target/wasm32-unknown-unknown/release/scratchmark.wasm
```

- `tools/wasm-smoke.mjs` 直接调 C ABI：建文档、写入、派生笔刷点、快照往返、撤销重做、局部擦除与切点标记、
  文档边界与整份平移、笔型预置、重采样。**53 项断言**，全部通过。
- 宿主不能自己 malloc，因此用 `sm_alloc` / `sm_free`（M18）在模块内申请缓冲区；`u64` 参数在 JS 侧必须传 BigInt。
- 已知限制：`wasm32-unknown-unknown` 上是 `panic = abort`，`guard()` 里的 `catch_unwind` 不再把 panic 变成 `SM_INTERNAL`，
  而是直接 trap。宿主应把 trap 当作"引擎不可用"处理。没有线程，`std::sync::Mutex` 退化为无竞争。
- 未验证：浏览器环境、SharedArrayBuffer/多线程、内存增长策略。

## Android

```powershell
pwsh tools/build-android.ps1            # 默认 API 24，产物在 dist/android
```

- 三个 ABI 都编出 `.so`，导出 **52 个 `sm_*`** 符号，`llvm-nm` 逐个核对过；`llvm-strip --strip-unneeded` 后 **0.32–0.49 MB/ABI**。
- 验证边界：**只证明"能编、符号对、依赖能解析"**。本机没有 Android 设备也没有模拟器，运行时行为未验证。
- 下一步（未做）：Kotlin 封装 + AAR（`jniLibs` 目录已按 Gradle 约定生成）、`System.loadLibrary("scratchmark")` 示例、
  真机/模拟器跑同一套冒烟（这需要你提供设备或允许装模拟器）。
- 已知限制：`armv7` 是 32 位，指针 4 字节；引擎内部的容量上限与 64 位一致，但快照最大 64 MiB 的分配在低内存设备上要留意。

## Linux

```powershell
cargo build -p scratchmark-ffi --release --target x86_64-unknown-linux-musl
```

- **只能出静态库** `libscratchmark.a`：`cdylib` 在 `x86_64-unknown-linux-musl` 上被 cargo 明确丢弃
  （`dropping unsupported crate type cdylib`），而 `.so` 需要有 Linux 链接器（本机没有，也没有 WSL 发行版）。
- 在有 Linux 的机器上产 `.so` 的命令：`cargo build -p scratchmark-ffi --release --target x86_64-unknown-linux-gnu`。
- 未验证：任何运行行为。

## Windows

见 `docs/SDK_README.md`：`tools/build-sdk.ps1` 打包，`tools/verify-sdk.ps1` 在源码树外解压并跑 C 动态 / C 静态 / C# 三套冒烟。

## iOS（未开始）

需要 macOS + Xcode 才能产出 `aarch64-apple-ios` / `aarch64-apple-ios-sim` 与 XCFramework。
在 Windows 上可以写 Swift 封装与 `Package.swift`，但**无法构建、无法验证**，所以本轮不做，避免交付"看起来有、其实没验证"的东西。

## .NET / NuGet

```powershell
pwsh tools/pack-nuget.ps1     # 先构建 SDK，再打包，再用临时工程装包验证
```

- 包内布局：`lib/net9.0/Scratchmark.dll` + `runtimes/win-x64/native/scratchmark.dll`；消费方 `new Engine()`（无参）
  会按 `SCRATCHMARK_DLL` → `runtimes/<rid>/native` → 程序目录 的顺序解析原生库。
- 验证：临时控制台工程从**本地源**装包，跑 11 项断言（写入 / 派生 / 快照 / 擦除切分与切点标记 / 边界 / 预置 / 原子剪切）全通过。
- 已知限制：只有 `win-x64` 原生资产；Android/iOS 的 RID 要等对应产物齐备后再加。包版本与 `Cargo.toml` 的 SDK 版本
  需要手工保持一致（`pack-nuget.ps1` 会核对它产出的 SDK zip 版本）。
- 踩过的坑（已写进脚本）：同名同版本的包会被 NuGet 全局缓存复用，验证前必须清 `~/.nuget/packages/scratchmark/<ver>`，否则验的是上一版。

## 宿主接入要点（跨平台一致）

1. 动态加载库，核对 `sm_abi_version() == 1`，再核对 `sm_features()` 覆盖你需要的位。
2. 只用 `sm_*` 导出；不要假设结构体以外的内存布局。
3. 每次编辑后看 `sm_get_state()` 的 `revision`；过期 revision 的写入会得到 `SM_STALE_REVISION`。
4. 快照是**自描述字节**，不要自己解析；跨平台字节序固定为小端。
5. 引擎不出位图：渲染、视口、文件事务、剪贴板系统集成都在宿主侧（见 `docs/HOST_INTEGRATION.md`）。
