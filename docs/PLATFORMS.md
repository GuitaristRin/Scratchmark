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
| Android | `jniLibs/<abi>/{libscratchmark,libscratchmark_jni}.so` + Kotlin 封装 + AAR 骨架 | 本机 + NDK 30 | ❌ 无设备/模拟器 | 产物与封装已生成，仅编译与符号核对 |
| Linux x86_64 | `libscratchmark.so`（glibc 2.17 基线，512 KB） | 本机 + zig | ❌ 无 Linux 运行时 | 已交付产物，仅编译 / 符号 / 依赖核对 |
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
pwsh tools/build-android.ps1            # 三个 ABI 的 C 层 + JNI 层 .so → dist/android/jniLibs
```

- 每个 ABI 出两个库：`libscratchmark.so`（C ABI，**52 个 `sm_*` 导出**）与 `libscratchmark_jni.so`
  （JNI 层，**24 个 `Java_com_scratchmark_Native_*` 导出**），都用 `llvm-nm` 逐个核对过；
  `llvm-strip --strip-unneeded` 后两库合计约 **0.67–1.0 MB/ABI**。
- Kotlin 封装在 `bindings/kotlin`（`Engine` / `Sample` / `BrushPoint` / `SampleFlags` / `BrushKind`），
  AAR 骨架已按 Gradle 约定放好 `jniLibs`、`build.gradle.kts`、`AndroidManifest.xml`、`consumer-rules.pro`。
- 验证边界：**只证明"能编、符号对"**。本机没有 Android 设备也没有模拟器在跑，运行时行为未验证；
  本机也没有 gradle/kotlinc，AAR 需要在你那边 `assembleRelease`。
- 已知限制：`Engine` 非线程安全（同实例别并发）；未发布 Maven；`armv7` 是 32 位目标。
## Linux

```powershell
pwsh tools/build-linux.ps1
```

- 有 zig 时：用 `zig cc -target x86_64-linux-gnu.2.17` 当交叉链接器，产出真正的 **`libscratchmark.so`**
  （ELF64 / DYN / x86-64，**52 个 `sm_*` 导出**，依赖只有 `libm`、`libc`、`ld-linux`、`libpthread`、`libdl`，strip 后 **512 KB**）。
- 没有 zig 时：退回 `x86_64-unknown-linux-musl` 静态库 `libscratchmark.a`——cargo 会明确
  `dropping unsupported crate type cdylib`，musl 目标出不了 `.so`。
- 验证等级：**编译 + 符号 + 依赖核对**，本机没有 Linux 运行时，**未验证运行**。
- zig 版本记录：0.18.0-dev.930（`%LOCALAPPDATA%\zig`）；脚本用 `ZIG_EXE` 或自动探测，不把绝对路径提交进仓库。
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

## 一次重跑全部四层

```powershell
pwsh tools/build-wasm.ps1      # 2.5 MB wasm + Node 54 项
pwsh tools/build-android.ps1   # 3 ABI × (52 个 C 导出 + 24 个 JNI 导出)
pwsh tools/build-linux.ps1     # glibc 2.17 的 .so，52 个导出 + 依赖核对
pwsh tools/pack-nuget.ps1      # nupkg + 消费方 11 项
```

最近一次重跑（2026-11-08）：四层全部 exit 0，产物见下表。

| 产物 | 大小 |
| --- | --- |
| `dist/wasm/scratchmark.wasm` | 2,521,256 |
| `dist/android/jniLibs/arm64-v8a/libscratchmark.so` + `_jni.so` | 467,008 + 451,856 |
| `dist/android/jniLibs/armeabi-v7a/libscratchmark.so` + `_jni.so` | 334,864 + 329,380 |
| `dist/android/jniLibs/x86_64/libscratchmark.so` + `_jni.so` | 509,048 + 495,976 |
| `dist/linux/libscratchmark.so` | 511,864 |
| `dist/linux/libscratchmark.a`（musl 静态，备用） | 24,998,854 |
| `dist/nuget/Scratchmark.0.18.0.nupkg` | 1,122,927 |
## 宿主接入要点（跨平台一致）

1. 动态加载库，核对 `sm_abi_version() == 1`，再核对 `sm_features()` 覆盖你需要的位。
2. 只用 `sm_*` 导出；不要假设结构体以外的内存布局。
3. 每次编辑后看 `sm_get_state()` 的 `revision`；过期 revision 的写入会得到 `SM_STALE_REVISION`。
4. 快照是**自描述字节**，不要自己解析；跨平台字节序固定为小端。
5. 引擎不出位图：渲染、视口、文件事务、剪贴板系统集成都在宿主侧（见 `docs/HOST_INTEGRATION.md`）。
