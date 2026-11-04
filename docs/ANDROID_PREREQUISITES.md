# Android 接入前置核查

日期：2026-11-04。状态：用户提供 NDK 后已完成部署与交叉编译探针；Android 引擎构建与运行仍未开始。当前继续 M2 主功能，参裁定 D5 / D7。

## 本机证据

- **SDK 已存在**：`D:/Applications/AndroidSDK`。其他项目的 `local.properties` 也指向这里；只读取 sdk.dir 字段，未修改那些项目。
- SDK 含 `platforms/android-36`、`platforms/android-37.0`，build-tools 34.0.0 / 36.0.0，以及 emulator、platform-tools 和系统镜像。`emulator.exe -version` 确认为 37.1.11.0；未启动模拟器或执行 adb。
- **Kotlin 已存在且可运行**：`D:/Applications/AndroidStudio/plugins/Kotlin/kotlinc/bin/kotlinc.bat -version` 返回 kotlinc-jvm 2.3.10，使用 Studio 内置 JBR 25.0.2。不在 PATH 不能视为未安装。
- **Gradle 已缓存**：`%USERPROFILE%/.gradle/wrapper/dists` 下存在 9.3.1 与 9.7.1 的完整 bin 目录；缓存还有 AGP 8.5.0。版本是否适合组合需单独核对，不因存在缓存就宣称 Android 工程能构建。
- `rustup target list --installed` 仍仅 Windows GNU；另查所有已安装 Rust toolchain 的 rustlib 子目录，均未发现 Android 标准库目标。
- **NDK 已部署**：用户提供 `C:/Users/mc158/Downloads/android-ndk-r30-windows/android-ndk-r30`，确认 r30 / 30.0.16248370 后复制至 `D:/Applications/AndroidSDK/ndk/30.0.16248370`。源目录保留，未重复下载、未改系统 PATH。
- 找到的 WinGet LLVM-MinGW `clang.exe` 面向 Windows，Studio 的 `plugins/android-ndk/*.jar` 是 IDE 插件，二者都不能充当 Android NDK。

初次只查 PATH 与常规 C 盘路径，遗漏 D 盘 SDK/Studio，结论过早。`rg --files` 默认还会跳过被 Git 忽略的 `local.properties`；复查使用 `--hidden --no-ignore` 并限定文件名。核查结果保存在 `artifacts/android-toolchain-20261104.json`。

## 当前部署证据与后续缺口

1. NDK 的 8,627 个文件相对路径 / 大小与源目录一致；source.properties、clang.exe、llvm-readelf.exe、API 头及 ARM64 API 26 libc.so 的 SHA-256 均相同。清单总字节数为 2,158,087,806。
2. 已部署 Clang 21.0.0 实际交叉编译 / 链接 API 26 的 ARM64 与 x86_64 共享库探针。探针静态断言通过当前 C 头文件布局，两种 ELF 的 LOAD 段均为 0x4000 对齐。证据：`artifacts/ndk-r30-deployment.json`、`ndk-r30-copy.log`、`ndk-r30-probe-*-elf.txt`。
3. Rust Android 标准库目标仍未安装；本次部署授权不扩大为网络安装或 Android 引擎运行。无需重装 SDK、Studio、Kotlin 或 Gradle。
4. JNI/Kotlin 实验规格与独立宿主验收、本仓 Android 构建入口仍需建立。探针不是 Scratchmark Rust SO，不构成 JNI、引擎运行或设备手感证据。

## 后续配置入口（暂缓）

- Gradle 宿主可用 `ndkVersion = "30.0.16248370"` 指定现有 side-by-side NDK；其他构建入口显式传 NDK 路径。本仓未修改其他项目配置。
- 后续需要 `aarch64-linux-android` 与按需的 `x86_64-linux-android` Rust 标准库；此前补齐方案已因用户暂缓 Android 而撤回，本轮未执行。
- API 26 仅用于本次 C 编译探针，不是产品 minSdk 裁定。JNI/Kotlin、真实引擎 SO 与设备运行后续单独验收。

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

后续还需实际构建 Rust 引擎 SO 并验证 JNI 加载、错误与编辑闭环。尚无引擎 SO、JNI、AAR、Android 引擎运行或设备手感证据。本轮只复制已有 NDK 并验证 C 探针，未下载组件、未连接设备、未写未编译的 JNI/Kotlin 占位实现。
