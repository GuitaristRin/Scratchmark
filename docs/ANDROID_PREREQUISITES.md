# Android 接入前置核查

日期：2026-11-04。状态：用户指出遗漏后已扩大查找并更正。Android 实现与运行未开始；不构成平台验收。

## 本机证据

- **SDK 已存在**：`D:/Applications/AndroidSDK`。其他项目的 `local.properties` 也指向这里；只读取 sdk.dir 字段，未修改那些项目。
- SDK 含 `platforms/android-36`、`platforms/android-37.0`，build-tools 34.0.0 / 36.0.0，以及 emulator、platform-tools 和系统镜像。`emulator.exe -version` 确认为 37.1.11.0；未启动模拟器或执行 adb。
- **Kotlin 已存在且可运行**：`D:/Applications/AndroidStudio/plugins/Kotlin/kotlinc/bin/kotlinc.bat -version` 返回 kotlinc-jvm 2.3.10，使用 Studio 内置 JBR 25.0.2。不在 PATH 不能视为未安装。
- **Gradle 已缓存**：`%USERPROFILE%/.gradle/wrapper/dists` 下存在 9.3.1 与 9.7.1 的完整 bin 目录；缓存还有 AGP 8.5.0。版本是否适合组合需单独核对，不因存在缓存就宣称 Android 工程能构建。
- `rustup target list --installed` 仍仅 Windows GNU；另查所有已安装 Rust toolchain 的 rustlib 子目录，均未发现 Android 标准库目标。
- **NDK 尚未定位**：现有 SDK 无 ndk / ndk-bundle 目录。扩大到 D 盘、用户目录、Program Files、C:/Applications、C:/tools、C:/msys64 的文件名查找，未发现 NDK 编译器或 NDK 压缩包；部分系统 / 应用目录拒绝读取，不能声称全盘绝对不存在。
- 找到的 WinGet LLVM-MinGW `clang.exe` 面向 Windows，Studio 的 `plugins/android-ndk/*.jar` 是 IDE 插件，二者都不能充当 Android NDK。

初次只查 PATH 与常规 C 盘路径，遗漏 D 盘 SDK/Studio，结论过早。`rg --files` 默认还会跳过被 Git 忽略的 `local.properties`；复查使用 `--hidden --no-ignore` 并限定文件名。核查结果保存在 `artifacts/android-toolchain-20261104.json`。

## 继续前需要的具体输入

1. NDK 的实际安装路径，或授权补齐 NDK 与 Rust Android 标准库目标。无需重装 SDK、Studio、Kotlin 或 Gradle。
2. JNI/Kotlin 实验规格与独立宿主验收，涵盖句柄释放、缓冲区容量 / 对齐、revision、快照和错误路径；先写规格再实现。
3. 本仓构建入口仍需建立，不能修改或复制其他项目的业务代码。设备运行另行授权；交叉构建不能代替运行。

## 组件补齐方案（待授权，未下载）

- 官方当前 LTS 为 NDK r30（30.0.16248370），Windows ZIP 为 727,980,788 字节，约 728 MB；[官方清单](https://developer.android.com/ndk/downloads?hl=en) 于本日核对。
- 拟下载到本仓 `artifacts/toolchains/`，核对官方校验值后解压，保持现有 SDK 安装不变；本地构建脚本通过参数指定 NDK 路径，不写系统 PATH。
- 仅为当前 Rust toolchain 补 `aarch64-linux-android` 与 `x86_64-linux-android` 标准库；分别用于 ARM64 产物与现有 x86_64 模拟器的后续验证准备。
- 本次编译实验拟用 API 26，不将其视为产品 minSdk 裁定。先完成 SO 构建、架构 / 依赖 / 页对齐检查，再推进 JNI/Kotlin 实际调用；不操作真机、不启动模拟器、不推送产物。
- M1 任务卡要求“若……Android 需安装组件，先记录具体缺口与可审查方案再请求决定”，故此方案未执行。

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

构建后还需记录编译器与源码身份、检查 SO 的 ELF 架构 / 依赖 / 页对齐，并实际验证 JNI 加载、错误与编辑闭环。尚无 SO、JNI、AAR、Android 运行或设备手感证据。本轮未安装组件、未连接设备、未写未编译的 JNI/Kotlin 占位实现。
