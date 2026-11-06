# Scratchmark

可嵌入的手写与页面批注模块。

Scratchmark 面向独立手写画布，以及课本、试卷等页面上的笔迹与批注。当前 SDK 0.7.0 提供实验核心、C ABI 与 .NET 9 C# 适配；能力与验证边界见 [当前状态](docs/STATE_2026-11-06.md)。

## 当前范围

- 批量样本输入、活动笔划、固定宽度折线与边界读取、提交 / 取消、整笔撤销重做。
- 可编辑快照导出 / 导入（格式 2 携带页面标识）；宿主执行实际文件读写与像素呈现。
- 可配置在线平滑、逐点压力笔宽的派生读取，原始样本保持；显示配置与视口由宿主保留。
- 页面标识与页面坐标 ↔ 视口坐标的纯函数变换；缩放平移后笔迹与页面内容对应。
- 原始中心线命中测试与整笔擦除；删除进入撤销历史，撤销恢复原 z 序。
- 沿擦除路径的局部擦除：按原始中心线切分笔划，片段继承宽度与颜色、占据原索引，一次手势只占一个撤销条目。
- 套索命中（只读查询）与按索引批量删除、清空；一次调用只占一个撤销条目，引擎不保存选区。
- 选区仿射变换（平移 / 缩放 / 旋转 / 镜像 / 剪切）与剪贴板式复制粘贴；剪贴板内容是格式 1 快照字节，可跨页使用。
- Windows 原生动态 / 静态与 .NET 9 C# 实验适配；Linux x86_64 GNU 动态 / 静态接入与 SDK 独立验收已复跑。设备前端与手感未验证。

PezMax-One 的课本／试卷批注是明确的后续接入场景。二进制 SDK 接入不必依赖 Git submodule，源码依赖作为另一种可选方式；本阶段不修改 PezMax-One，也不建立 submodule。

## 开发与验证

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
.\tools\build-sdk.ps1
```

Windows SDK 内附 `verify-sdk.ps1`，可在仅有包内容与 GCC 的目录验证动态加载及静态链接，不要求检出引擎源码。Linux x86_64 GNU 使用 `tools/build-sdk.sh` 与 `tools/verify-sdk.sh`，动态与静态接入、导出清单与快照一致性已在本机独立宿主验收；Android 尚未验收。接口与快照均为实验版，不代表正式兼容承诺。

加上 `-IncludeCSharp` 可用已有 .NET 9 SDK 离线构建并运行 C# 宿主，验证共享夹具、编辑与恢复、布局、错误路径和释放时序；调用契约见 [C# 实验规格](spec/M1-CSHARP.md)。Android 的具体缺口见 [前置核查](docs/ANDROID_PREREQUISITES.md)。

PezMax-One 重构可参考 [宿主接入说明](docs/HOST_INTEGRATION.md)，其中明确页面坐标、输入生命周期、笔刷配置保存责任及未验证平台。Android 当前暂缓，NDK 已按用户提供的包部署。

## 本地参考资料

完整 主流移动便签分析资料保存在 `本地参考/`，包括外部行为观察代码、APK、原生库、工具与已有分析文档。整个 `Refs/` 已被 Git 忽略，不随仓库推送。

Git 远端使用 SSH：`git@github.com:GuitaristRin/Scratchmark.git`。

## 文档

- [立项任务卡](docs/PROJECT_BRIEF.md)
- [产品与架构方向裁定 - Canon](docs/DECISIONS_2026-11-03.md)
- [M0 实施任务卡 - Canon](docs/IMPLEMENTATION_M0.md)
- [M1 宿主适配任务卡 - Canon](docs/IMPLEMENTATION_M1.md)
- [M2 平滑与压力笔刷任务卡 - Canon](docs/IMPLEMENTATION_M2.md)
- [M2 派生几何规格 - Canon](spec/M2.md)
- [M3 页面标识与视口变换任务卡 - Canon](docs/IMPLEMENTATION_M3.md)
- [M3 页面标识与视口规格 - Canon](spec/M3.md)
- [M4 命中测试与整笔擦除任务卡 - Canon](docs/IMPLEMENTATION_M4.md)
- [M4 命中与擦除规格 - Canon](spec/M4.md)
- [M5 局部擦除任务卡 - Canon](docs/IMPLEMENTATION_M5.md)
- [M5 局部擦除规格 - Canon](spec/M5.md)
- [M6 选择与批量删除任务卡 - Canon](docs/IMPLEMENTATION_M6.md)
- [M6 选择与批量删除规格 - Canon](spec/M6.md)
- [M7 选区变换任务卡 - Canon](docs/IMPLEMENTATION_M7.md)
- [M7 选区变换与复制粘贴规格 - Canon](spec/M7.md)
- [M0 实验规格 - Canon](spec/M0.md)
- [C 接口声明](include/scratchmark.h)
- [SDK 使用说明](docs/SDK_README.md)
- [当前状态](docs/STATE_2026-11-06.md)
- [主流移动便签样本探索](docs/REFERENCE_FINDINGS.md)
- [多输入与语言选型提案 - Draft](docs/PROPOSALS_2026-11-03.md)
- [输入与宿主约束核查 - Draft](docs/research/2026-11-03-input-and-host-evidence.md)
- [协作约定](AGENTS.md)

按已确认的纯引擎及 Rust 核心 / 原生适配方向继续设计；具体依赖、显示后端、文件格式与对外接口仍待研究后确定。仓库当前计划保持私有，待研发充分后再评估公开。
