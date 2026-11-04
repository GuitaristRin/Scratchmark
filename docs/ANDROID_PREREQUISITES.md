# Android 接入前置核查

日期：2026-11-04。状态：环境核查完成，Android 实现与运行未开始；不构成平台验收。

## 本机证据

- `rustup target list --installed`：仅 `x86_64-pc-windows-gnu`，未安装 Android Rust 标准库目标。
- `javac -version`：17.0.12，存在 JDK；`adb` 在 PATH。未执行 adb 命令，未连接或操作设备。
- `Get-Command kotlinc,sdkmanager,clang` 未找到命令；存在 GCC 与 CMake，但 Windows GCC 不能代替 Android NDK 编译器。
- `ANDROID*`、`JAVA_HOME`、`KOTLIN*` 环境变量未设置。
- 常规路径 `%LOCALAPPDATA%/Android`、`C:/Android`、`D:/Android`、`C:/Applications/Android`、`C:/Program Files/Android` 未发现 SDK/NDK。此项仅覆盖这些路径，不能证明其他磁盘或自定义路径不存在工具链。
- .NET 无已安装工作负载，不能据 Windows C# 运行成功声称 Android 已可运行。

## 继续前需要的具体输入

1. 已有 Android SDK/NDK 的绝对路径与版本，或另行授权取得组件；Rust 的 `aarch64-linux-android` 目标也需准备。先以 arm64-v8a 验证，模拟器目标另计。
2. 指定一次实验所用的 minSdk API；它是 NDK 编译器目标后缀，尚未确定为产品最低版本。
3. 已有 Kotlin / Android 构建工具及版本。当前仓没有 Gradle wrapper、AGP 或 Kotlin 项目，不从别仓复制、不自行新增下载依赖。
4. JNI/Kotlin 适配规格与独立宿主验收，涵盖句柄释放、缓冲区容量 / 对齐、revision、快照和错误路径。设备运行仍需另行授权；交叉构建不能代替运行。

依据 [Rust Android target 文档](https://doc.rust-lang.org/rustc/platform-support/android.html)，该目标从宿主交叉编译并使用 Android NDK；[Android NDK 官方说明](https://developer.android.com/ndk/guides/other_build_systems) 给出了 Windows 的 `windows-x86_64` 工具目录和 arm64 的 `aarch64-linux-android` 目标，Clang 目标后需加本次实验的 API 后缀。两处已于本日核对；本轮没有固定 NDK/SDK/Kotlin 版本。

## 后续构建命令草案（本轮未运行）

以下只说明原生引擎构建入口；并非已验收脚本，不生成 JNI 桥或 Kotlin 宿主。`$ndkRoot` 与 `$minimumApi` 必须由已确认的本地工具链和实验配置给定。

```powershell
# 取得目标标准库会访问网络；本轮未授权安装，未执行。
rustup target add aarch64-linux-android

# 在独立构建终端使用；避免影响 Windows GNU 构建。
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $ndkRoot 'toolchains/llvm/prebuilt/windows-x86_64/bin/clang.exe'
$env:RUSTFLAGS = "-C link-arg=--target=aarch64-linux-android$minimumApi"
cargo build -p scratchmark-ffi --release --locked --target aarch64-linux-android
```

构建后还需记录编译器与源码身份、检查 SO 的 ELF 架构 / 依赖 / 页对齐，并实际验证 JNI 加载、错误与编辑闭环。尚无 SO、JNI、AAR、Android 运行或设备手感证据。本轮遵守 M1 任务卡，未安装组件、未连接设备、未写未编译的 JNI/Kotlin 占位实现。
