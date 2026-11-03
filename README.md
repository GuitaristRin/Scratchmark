# Scratchmark

可嵌入的手写与页面批注模块。

Scratchmark 面向独立手写画布，以及课本、试卷等页面上的笔迹与批注。项目处于立项和技术研究阶段，尚未实现手写引擎。

## 当前范围

- 梳理输入、笔迹渲染、编辑工具、页面坐标与持久化的职责边界。
- 研究已有成熟样本，区分已核对事实、算法线索和待验证假设。
- 为后续实现建立可复现的验证方法。

PezMax-One 的课本／试卷批注是明确的后续接入场景。未来可以通过 Git submodule 集成；本阶段不修改 PezMax-One，也不建立 submodule。

## 本地参考资料

完整 主流移动便签分析资料保存在 `本地参考/`，包括外部行为观察代码、APK、原生库、工具与已有分析文档。整个 `Refs/` 已被 Git 忽略，不随仓库推送。

Git 远端使用 SSH：`git@github.com:GuitaristRin/Scratchmark.git`。

## 文档

- [立项任务卡](docs/PROJECT_BRIEF.md)
- [产品与架构方向裁定 - Canon](docs/DECISIONS_2026-11-03.md)
- [主流移动便签样本探索](docs/REFERENCE_FINDINGS.md)
- [多输入与语言选型提案 - Draft](docs/PROPOSALS_2026-11-03.md)
- [输入与宿主约束核查 - Draft](docs/research/2026-11-03-input-and-host-evidence.md)
- [协作约定](AGENTS.md)

按已确认的纯引擎及 Rust 核心 / 原生适配方向继续设计；具体依赖、显示后端、文件格式与对外接口仍待研究后确定。仓库当前计划保持私有，待研发充分后再评估公开。
