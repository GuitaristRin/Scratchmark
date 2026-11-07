# 验证矩阵

一条原则：**只有真跑过才算验证**。编译通过、符号存在、文档写着——都不算。本文件记录每一层的实际状态与补验方法。

## 汇总

| 层 | 断言/检查数 | 等级 | 证据来源 |
| --- | --- | --- | --- |
| Rust workspace（引擎 + ABI + JNI） | **220 项** | ✅ 真跑 | `cargo test --workspace` |
| Windows C 动态加载 | **677 项** | ✅ 真跑 | SDK 内 `verify-sdk.ps1`（源码树外） |
| Windows C 静态链接 | **677 项** | ✅ 真跑 | 同上（`SM_STATIC` 路径） |
| Windows C# 宿主 | **547 项** | ✅ 真跑 | 同上 `-IncludeCSharp` |
| 便签验证器（仓库外 GUI） | **129 项** | ✅ 真跑 | `Scratchmark.Verifier --selftest` |
| WebAssembly（Node） | **54 项** | ✅ 真跑 | `tools/wasm-smoke.mjs` |
| .NET NuGet 消费方 | **11 项** | ✅ 真跑 | `tools/pack-nuget.ps1` |
| Android 三 ABI | 52 C 导出 + 24 JNI 导出 × 3 | ⚠️ 编译 + 符号 | `tools/build-android.ps1` |
| Linux x86_64 | 52 导出 + 依赖清单 | ⚠️ 编译 + 符号 | `tools/build-linux.ps1` |

数字来自 2026-11-08 最后一次重跑；改代码后以当次输出为准，并把新数字写回本文件。

## 各项怎么复现

```powershell
# 引擎层
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# Windows SDK（含 C 动态 / C 静态 / C#）
pwsh tools/build-sdk.ps1
# 解压到临时目录后：
pwsh <解压目录>/verify-sdk.ps1 -IncludeCSharp

# WebAssembly
pwsh tools/build-wasm.ps1          # 内部就跑 wasm-smoke.mjs

# Android（只编 + 符号）
pwsh tools/build-android.ps1

# Linux（只编 + 符号 + 依赖）
pwsh tools/build-linux.ps1

# .NET 包（打包 + 消费方真跑）
pwsh tools/pack-nuget.ps1
```

## 断言覆盖什么

- **Rust workspace（220）**：核心几何与编辑语义（切分、变换、形状拟合、预测、重采样、预置）、快照编解码与格式兼容、
  撤销重做与预算、以及 C ABI 的指针/容量/错误码路径、JNI 层编译期约束。
- **C 677**：与 C# 共用同一份行为断言（`examples/c/smoke.c`），逐里程碑：输入/派生/撤销/命中/擦除/选区/变换/形状/
  视口/预测/轴向/边界/预置/重采样/刷新率/剪切/切点标记/宿主分配。**动态与静态两条链接路径各跑一遍**。
- **C# 547**：绑定层的编组、错误映射、快照往返、剪贴板、释放时序；`-IncludeCSharp` 时与 C 宿主比对同一份快照。
- **验证器 129**：真实渲染 + 交互层面的检查（含局部擦除的观感对比、切点圆缺口）。
- **WASM 54**：先在 wasm 里核对 28 个必需导出，再跑一遍编辑闭环（写入/派生/快照/撤销/擦除切分与切点/边界/平移/预置/重采样）。

## 明确未验证

| 项 | 为什么没验证 | 怎么补 |
| --- | --- | --- |
| Android 运行时（JNI 真调用） | 本机没有设备；`Pixel_10` AVD 存在但未启动 | 启动模拟器后跑一份等价于 `wasm-smoke` 的 Kotlin 冒烟 |
| Android AAR 组装 | 本机没有 gradle / kotlinc | 在装了 Android Studio 的机器 `assembleRelease` |
| Android 15+ 的 16 KB 页对齐 | 未核对 `.so` 的 LOAD 段对齐（镜像带 `ps16k`） | `llvm-readelf -l` 检查 LOAD 对齐是否 ≥ 16 KB |
| Linux 运行时 | 本机没有 Linux 运行时，也没有 WSL 发行版 | 在任意 Linux 上 `dlopen` 后跑 C 冒烟 |
| iOS / macOS | 需要 macOS + Xcode | 有机器后出 XCFramework |
| 真机手写笔的倾斜/压力读数 | 需要设备 | 采集真实样本后核对 `tilt ∈ [0, π/2]`、`orientation ∈ [-π, π]` |
| 观感（笔触手感、预置档位是否"好看"） | 主观且需要设备 | 人眼在真机上调 |
| 超大文档性能 | 只有正确性测试，没有基准 | 加 benchmark（例如 10 万点下的擦除/平移耗时） |
| 浏览器环境 / WASM 多线程 | 只在 Node 单线程验证 | 浏览器 + SharedArrayBuffer 场景另行验证 |
| 跨线程并发调用同一文档 | 只有“每文档一把锁”的结构保证，没有并发测试 | 写 stress 测试 |

## 报告规范

1. 说"已验证"时必须给**项数**与**复现命令**。
2. 只编过、只核过符号的，写"可编译 / 符号核对"，不写"支持"。
3. 没跑过的写"未验证"，并说明卡在哪（缺设备 / 缺工具链 / 缺运行时）。
4. 每次改完代码，回头更新本文件的数字；数字对不上就是文档失真。
