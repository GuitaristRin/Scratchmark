# Scratchmark

可嵌入的手写与页面批注模块。

Scratchmark 面向独立手写画布，以及课本、试卷等页面上的笔迹与批注。当前提供 M0 实验核心与 C ABI；能力与验证边界见 [当前状态](docs/STATE_2026-11-04.md)。

## 当前范围

- 批量样本输入、活动笔划、固定宽度折线与边界读取、提交 / 取消、整笔撤销重做。
- 可编辑快照导出 / 导入；宿主执行实际文件读写与像素呈现。
- 独立动态库加载与 SDK 打包验证；平滑、压力笔刷、设备前端与手感仍未实现。

PezMax-One 的课本／试卷批注是明确的后续接入场景。二进制 SDK 接入不必依赖 Git submodule，源码依赖作为另一种可选方式；本阶段不修改 PezMax-One，也不建立 submodule。

## 开发与验证

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
.\tools\build-sdk.ps1
```

Windows SDK 内附 `verify-sdk.ps1`，可在仅有包内容与 GCC 的目录验证动态加载及静态链接，不要求检出引擎源码。Linux x86_64 GNU 已有独立动态宿主验收，使用 `tools/build-sdk.sh` 与 `tools/verify-sdk.sh`；Linux 静态接入和 Android 尚未验收。接口与快照均为实验版，不代表正式兼容承诺。

## 本地参考资料

完整 主流移动便签分析资料保存在 `本地参考/`，包括外部行为观察代码、APK、原生库、工具与已有分析文档。整个 `Refs/` 已被 Git 忽略，不随仓库推送。

Git 远端使用 SSH：`git@github.com:GuitaristRin/Scratchmark.git`。

## 文档

- [立项任务卡](docs/PROJECT_BRIEF.md)
- [产品与架构方向裁定 - Canon](docs/DECISIONS_2026-11-03.md)
- [M0 实施任务卡 - Canon](docs/IMPLEMENTATION_M0.md)
- [M0 实验规格 - Canon](spec/M0.md)
- [C 接口声明](include/scratchmark.h)
- [SDK 使用说明](docs/SDK_README.md)
- [当前状态](docs/STATE_2026-11-04.md)
- [主流移动便签样本探索](docs/REFERENCE_FINDINGS.md)
- [多输入与语言选型提案 - Draft](docs/PROPOSALS_2026-11-03.md)
- [输入与宿主约束核查 - Draft](docs/research/2026-11-03-input-and-host-evidence.md)
- [协作约定](AGENTS.md)

按已确认的纯引擎及 Rust 核心 / 原生适配方向继续设计；具体依赖、显示后端、文件格式与对外接口仍待研究后确定。仓库当前计划保持私有，待研发充分后再评估公开。
