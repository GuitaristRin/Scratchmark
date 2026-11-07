# M19 - Android JNI 封装任务卡

状态：Canon（本阶段边界）。日期：2026-11-08。行为真源为先行编写的 `spec/M19.md` 与 `include/scratchmark.h`。
触发来源：目标 ② —— Android 需要 Kotlin 可调的封装，而 C ABI 只能给 C/JNI 直接调。

## 本轮范围

1. 新 crate `crates/scratchmark-jni`：24 个 JNI 导出，全部走 `catch_unwind`，只做编组。
2. Kotlin 封装 `bindings/kotlin`：`Engine`（AutoCloseable）、`Sample`、`BrushPoint`、`SampleFlags`、`BrushKind`。
3. AAR 骨架：`build.gradle.kts` + `AndroidManifest.xml` + `jniLibs` 约定 + `consumer-rules.pro`。
4. `tools/build-android.ps1` 扩展：同时构建 JNI 层、strip、核对 C 层 52 个与 JNI 层 24 个导出。
5. 文档：`bindings/kotlin/README.md`、`docs/PLATFORMS.md` 的 Android 一节。

## 验收

- `cargo build -p scratchmark-jni`（Windows，本机可编）+ 三个 Android ABI 交叉编译通过。
- 每个 ABI 的导出用 `llvm-nm` 核对：52 个 `sm_*` + 24 个 `Java_com_scratchmark_Native_*`。
- workspace 测试与 clippy 不因新增 crate 变红。
- 明确记录：**运行时未验证**（无设备/模拟器），且未做 Maven 发布。

## 止损与边界

不在 JNI 层复制引擎逻辑；不做多线程封装、Bitmap 渲染、Maven 发布；不修改 PezMax-One、不建 submodule。
