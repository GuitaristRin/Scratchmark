# Scratchmark

> **实验版（0.1.0-pre）**：接口与快照格式不承诺兼容。**已验证**与**明确未验证**的项目见 [验证矩阵](docs/VERIFICATION.md)，未验证项不允许表述为"支持"。

可嵌入的手写与页面批注模块。

Scratchmark 面向独立手写画布，以及课本、试卷等页面上的笔迹与批注。当前 SDK 0.1.0 提供实验核心、C ABI 1（52 个导出）、.NET 9 C# 适配、Android JNI 与 Kotlin 封装、WebAssembly 与 Linux 产物。**先看 [文档地图](docs/README.md)**；能力与验证边界见 [平台与封装库](docs/PLATFORMS.md) 与 [验证矩阵](docs/VERIFICATION.md)。

## 当前范围

- 批量样本输入、活动笔划、固定宽度折线与边界读取、提交 / 取消、整笔撤销重做。
- 可编辑快照导出 / 导入（格式 2 携带页面标识）；宿主执行实际文件读写与像素呈现。
- 可配置在线平滑、逐点压力笔宽的派生读取，原始样本保持；显示配置与视口由宿主保留。
- 页面标识与页面坐标 ↔ 视口坐标的纯函数变换；缩放平移后笔迹与页面内容对应。
- 原始中心线命中测试与整笔擦除；删除进入撤销历史，撤销恢复原 z 序。
- 沿擦除路径的局部擦除：按原始中心线切分笔划，片段继承宽度与颜色、占据原索引，一次手势只占一个撤销条目。
- 套索命中（只读查询）与按索引批量删除、清空；一次调用只占一个撤销条目，引擎不保存选区。
- 选区仿射变换（平移 / 缩放 / 旋转 / 镜像 / 剪切）与剪贴板式复制粘贴；剪贴板内容是格式 1 快照字节，可跨页使用。
- 形状规整：手写笔划拟合成直线 / 矩形 / 圆 / 箭头，带预览模式与残差，采纳与否由宿主决定。
- 带旋转的视口：页面 ↔ 视口换算、线宽与旋转外接框，另有锚点缩放与适应窗口两个纯函数；旧视口语义不变。
- 预测笔迹：活动笔划尾段的确定性外推（时间窗、长度上限、越界截断），只读、不进快照。
- 逐点轴向：倾斜与朝向随样本保存、分段读取，并随切分 / 变换 / 复制粘贴传播；快照格式 3 承载它。
- 文档边界与整份平移：内容并集边界（含笔宽）与一次调用一个撤销条目的整体位移；页面尺寸仍由宿主保留。
- 笔型尺寸预置：五笔型 × 五档位的归一化比例与宽度换算，供宿主统一默认手感。
- 点间距重采样：按「笔宽 × 比例」沿中心线等距取点，附带逐点笔宽，末点保留原始末点。
- 预测刷新率：按屏幕刷新率压缩预测时间窗；刷新率为 0 时与 M10 逐点一致。
- 原子剪切：一次调用产出剪贴板字节并删除选区，只产生一个撤销条目。
- 擦除切点标记：`flags` 位域标出被切出来的端点，宿主据此画圆缺口而不是平口。
- 宿主内存分配：`sm_alloc` / `sm_free`，让没有 malloc 的宿主（如 WebAssembly）也能用快照类接口。
- Windows 原生动态 / 静态与 .NET 9 C# 实验适配；Linux x86_64 GNU 动态 / 静态接入与 SDK 独立验收已复跑。设备前端与手感未验证。

PezMax-One 的课本／试卷批注是明确的后续接入场景。二进制 SDK 接入不必依赖 Git submodule，源码依赖作为另一种可选方式；本阶段不修改 PezMax-One，也不建立 submodule。

## 开发与验证

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
.\tools\build-sdk.ps1
```

Windows SDK 内附 `verify-sdk.ps1`，可在仅有包内容与 GCC 的目录验证动态加载及静态链接（C 677 项 ×2、C# 547 项），不要求检出引擎源码。Linux x86_64 用 `tools/build-sdk.sh` 与 `tools/verify-sdk.sh`；Android 与 WASM 的产物、构建脚本与验证等级见 [PLATFORMS.md](docs/PLATFORMS.md)。接口与快照均为实验版，不代表正式兼容承诺。

加上 `-IncludeCSharp` 可用已有 .NET 9 SDK 离线构建并运行 C# 宿主，验证共享夹具、编辑与恢复、布局、错误路径和释放时序；调用契约见 [C# 实验规格](spec/M1-CSHARP.md)。Android 的具体缺口见 [前置核查](docs/ANDROID_PREREQUISITES.md)。

PezMax-One 重构可参考 [宿主接入说明](docs/HOST_INTEGRATION.md)：页面坐标、输入生命周期、笔刷配置与页面尺寸的责任划分，以及未验证平台清单。Android 已交付三 ABI 的 `.so`、JNI 层与 Kotlin 封装，但**运行时未验证**（本机无设备/模拟器）。

## 本地参考资料

- 可选参考资料放在 `Refs/`（已被 Git 忽略，不随仓库推送）；本仓库的文档与代码不依赖它。

Git 远端使用 SSH：`git@github.com:GuitaristRin/Scratchmark.git`。

## 文档

入口是 [文档地图](docs/README.md)，按"规格 / 裁定 / 怎么干活 / 现在到哪了"四类组织。常用入口：

- [文档地图](docs/README.md) —— 阅读顺序与全部分类
- [架构](docs/ARCHITECTURE.md) —— 分层、数据模型、不变量、快照格式、能力位、错误码
- [开发手册](docs/DEVELOPMENT.md) —— 环境、四条门、四平台脚本、提交约定、已踩过的坑
- [验证矩阵](docs/VERIFICATION.md) —— 每层跑过多少项、什么没验证、怎么补
- [平台与封装库](docs/PLATFORMS.md) —— Windows / WASM / Android / Linux / .NET 的产物与验证等级
- [接口索引](docs/API.md) + [C 接口声明](include/scratchmark.h) —— 52 个导出的分组与共性约定
- [SDK 使用说明](docs/SDK_README.md)、[宿主接入说明](docs/HOST_INTEGRATION.md)
- [产品与架构方向裁定](docs/DECISIONS_2026-11-03.md)（`D1`…`D25`，只追加）、[立项任务卡](docs/PROJECT_BRIEF.md)
- 能力规格 `spec/M0.md` … `spec/M19.md`；当轮任务卡 `docs/IMPLEMENTATION_M<n>.md`
- [当前状态时间线](docs/README.md#四现在到哪了时间线)

按已确认的纯引擎及 Rust 核心 / 原生适配方向继续设计。本仓库自 0.1.0-pre 起公开；接口与快照格式仍是实验版，不承诺兼容。

## 许可

本项目采用 **MIT 或 Apache-2.0 双许可**，你可任选其一：

- [Apache License 2.0](LICENSE-APACHE)
- [MIT License](LICENSE-MIT)
- 第三方组件与归属见 [NOTICE](NOTICE)（当前唯一第三方依赖是 `jni`，同样为 MIT/Apache-2.0）

发布状态与未验证范围见 [docs/VERIFICATION.md](docs/VERIFICATION.md)；发布前遗留事项见 [docs/RELEASE_CHECKLIST.md](docs/RELEASE_CHECKLIST.md)。
