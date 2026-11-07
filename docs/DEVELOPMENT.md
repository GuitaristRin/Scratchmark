# 开发手册

面向要改这个仓库的人（包括几个月后的我自己）。只写能核对的命令与事实；数字都来自本机实跑。

## 环境

| 工具 | 本机实测 | 用途 |
| --- | --- | --- |
| Rust stable | 1.95.0，`x86_64-pc-windows-gnu` | 引擎、C ABI、JNI 层 |
| rustup 目标 | 见下表 | 交叉编译 |
| llvm-mingw clang | `clang.exe`（winget 包） | Windows gnu 链接 |
| Android NDK | 30.0.16248370，`D:\Applications\AndroidSDK\ndk` | Android 交叉编译与 ELF 核对 |
| zig | 0.18.0-dev.930，`%LOCALAPPDATA%\zig` | Linux `.so` 的交叉链接器 |
| Node | v25.8.1 | WASM 冒烟 |
| .NET SDK | 9.0.316 | C# 绑定、NuGet 包、验证器 |
| JDK | `java`/`javac` 在 PATH | 只是为了 JNI 符号；**没有 gradle/kotlinc** |

已安装的 rustup 目标（`rustup target list --installed`）：

```
x86_64-pc-windows-gnu        wasm32-unknown-unknown      x86_64-unknown-linux-gnu
x86_64-unknown-linux-musl    aarch64-linux-android       armv7-linux-androideabi
x86_64-linux-android
```

缺的：WSL 发行版、gradle、kotlinc、wasm-opt、iOS 工具链、macOS。**没有它们的功能就不能声称已验证**（见 [验证矩阵](VERIFICATION.md)）。

## 四条门（提交前必须全绿）

```powershell
cargo fmt --all -- --check
cargo test --workspace                                   # 220 项
cargo clippy --workspace --all-targets -- -D warnings
cargo build --release --workspace
```

再加一次 SDK 独立验收（在源码树外解压、用系统 GCC 与 .NET 构建）：

```powershell
pwsh tools/build-sdk.ps1                       # 产出 dist/scratchmark-<ver>-<target>-<hash>.zip
# 解压到临时目录后：
pwsh <解压目录>/verify-sdk.ps1 -IncludeCSharp  # C 动态 662 / C 静态 662 / C# 547
```

Windows 上还有仓库外的临时验证器（`..\Scratchmark-Verifier`），它不是本仓的一部分：
`Scratchmark.Verifier.exe --selftest --dll <sdk>/bin/scratchmark.dll`，当前 **129 项**。

## 四平台构建脚本

```powershell
pwsh tools/build-wasm.ps1      # wasm32 → dist/wasm/scratchmark.wasm，并跑 Node 冒烟（54 项）
pwsh tools/build-android.ps1   # 三个 ABI → dist/android/jniLibs，核对 52 个 C + 24 个 JNI 导出
pwsh tools/build-linux.ps1     # zig 交叉链接 → dist/linux/libscratchmark.so，核对符号与依赖
pwsh tools/pack-nuget.ps1      # → dist/nuget/Scratchmark.<ver>.nupkg，并用临时工程装包验收
```

每层的验证等级与已知限制见 [PLATFORMS.md](PLATFORMS.md)，矩阵见 [VERIFICATION.md](VERIFICATION.md)。

## 代码结构

| 路径 | 作用 | 约束 |
| --- | --- | --- |
| `crates/scratchmark-core` | 全部引擎语义 | **不许有窗口、文件、网络、线程调度**；只依赖 std |
| `crates/scratchmark-ffi` | C ABI（`cdylib` + `staticlib`） | 只做编组：句柄表、`catch_unwind`、指针校验 |
| `crates/scratchmark-jni` | JNI 层，24 个导出 | 只做编组，语义来自 core；自带句柄表 |
| `bindings/csharp` | P/Invoke 绑定 | 不复制语义；`Engine()` 无参会按 RID 解析原生库 |
| `bindings/kotlin` | Kotlin 封装 + AAR 骨架 | 同上 |
| `examples/c`、`examples/csharp` | 冒烟与用法示例 | 与 `verify-sdk` 共用断言，不许只验证“能链接” |
| `spec/M*.md` | 行为真源 | **先写规格再写代码** |
| `tools/*.ps1`、`tools/*.sh` | 构建与验收 | 每层都要能一条命令重跑 |

## 工作约定

1. **规格先行**：新能力先落 `spec/M<n>.md`，再写 `docs/IMPLEMENTATION_M<n>.md` 任务卡，然后才是代码。
2. **断言不放宽**：测试红了先判断是实现错还是**契约变了**。属实契约演进（例如 M17 让切点带 `flags` 的 bit1），
   要在提交信息与 STATE 里写明“这是契约演进”，并同步改规格；不许为了变绿而删断言或改预期。
3. **证据分级**：能跑出来的写“真跑 + 项数”；只编过的写“可编译 + 符号核对”；没做的写“未验证”。
   不许把“能编”写成“支持”（见 [PLATFORMS.md](PLATFORMS.md) 的三层）。
4. **不虚构**：报告数字必须来自当次工具输出；没跑过的不写。
5. **裁定追加**：`docs/DECISIONS_2026-11-03.md` 只追加 `D<n>`，不改旧条目。
6. **状态记录**：每轮结束往 `docs/STATE_<日期>.md` 追加一节（做了什么、证据、失败、未验证）。
7. **`Refs/`**：被 Git 忽略的本地参考目录，不进版本控制、不在文档里当正文引用。
8. **提交信息**：中文标题 + 说明“为什么”，正文列关键取舍；不写机器路径。

## 已踩过的坑（别再踩一遍）

| 坑 | 现象 | 处理 |
| --- | --- | --- |
| rustup 镜像 | `rustup target add` 报 403，`RUSTUP_DIST_SERVER` 不生效 | 工具链清单里 1140 处清华绝对 URL 被缓存；把 `multirust-channel-manifest.toml` 里的 `mirrors.tuna.tsinghua.edu.cn/rustup/dist/` 换成 `static.rust-lang.org/dist/`（原文件备份 `*.tuna-backup`） |
| 目标目录残留 | `detected conflict: lib\rustlib\x86_64-unknown-linux-gnu\...` | 删掉该目录后重装目标 |
| musl 出不了 `.so` | cargo 警告 `dropping unsupported crate type cdylib` | 换 `x86_64-unknown-linux-gnu` + zig 链接；musl 只出静态库 |
| NuGet 缓存 | 重打包后验收跑的还是旧库 | 同 id+版本会命中 `~/.nuget/packages/<id>/<ver>`；`pack-nuget.ps1` 每次先删 |
| 嵌套调用退出码 | `build-sdk.ps1` 在别的脚本里 `$LASTEXITCODE` 不可靠 | 不依赖退出码，核对它产出的 zip |
| `#[cfg(test)]` 重复 | `mod xxx_tests;` 落到 lib 里，clippy 报 dead code | 每个 test 模块前只留一个 `#[cfg(test)]` |
| PowerShell 文本替换 | 带 CRLF 的文件里 `Replace` 静默不生效 | 先 `read` 确认原文，或用 `edit` 工具，或按行号改 |
| `Select-Object -Unique` | 曾把 `smoke.c` 里重复的空行与列表项一起删掉 387 行 | 不要用它去重文件内容；用 git 恢复后逐行重建 |
| WASM 的 `u64` | JS 里传 `1` 报 `Cannot convert 1 to a BigInt` | 所有 `u64` 参数/返回值在 JS 侧用 BigInt |
| Android 32 位 | `armv7` 指针 4 字节 | 引擎限制值与 64 位一致，但 64 MiB 快照在低内存设备上要当心 |

## 常用命令速查

```powershell
# 单测一个模块
cargo test -p scratchmark-core --test erase_path

# 只看引擎层的失败
cargo test --workspace 2>&1 | Select-String "FAILED|panicked"

# 验证器截图（仓库外）
..\Scratchmark-Verifier\bin\Debug\net9.0-windows\Scratchmark.Verifier.exe --demo --capture out.png --size 900x760 --dll <sdk>\bin\scratchmark.dll

# 提交前一次性
cargo fmt --all -- --check; cargo test --workspace; cargo clippy --workspace --all-targets -- -D warnings
```
