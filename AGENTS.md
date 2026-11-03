<!-- BEGIN workbench-pointer · v1 · 勿手改本块（由 GuitaristRin/Projects 统一维护；改模板请改 templates/repo-AGENTS-block.md） -->
> **本文件只写本仓特有的规则。跨仓通用约定不在本仓重复**，唯一真源是工作台文档库：
> - Windows：`%USERPROFILE%\Documents\Projects\AGENTS.md`
> - Linux：`~/projects/AGENTS.md` → `~/projects/Arcturius/AGENTS.md`（按序取第一个存在的）
> - 远端：<https://github.com/GuitaristRin/Projects/blob/main/AGENTS.md>
>
> 动手之前先读它。**读不到就在报告里写明「工作台未读到」**，然后按本文件执行，不要猜。
> 新发现的通用规则先回流工作台（`docs/09-回流与同步.md`），本文件只留引用，不抄正文。
> 冲突优先级：用户当前明确指令 > 本仓特有规则 > 工作台通用规则。
<!-- END workbench-pointer -->

# Scratchmark 特例

- 当前阶段与已确认范围以 docs/PROJECT_BRIEF.md 为准。
- PezMax-One 仅作为后续接入场景；本阶段不修改该仓、不建立 submodule。
- 已确认方向与实施边界见 `docs/DECISIONS_2026-11-03.md` 及 `docs/IMPLEMENTATION_M0.md`；实验接口以 `spec/M0.md` 和 `include/scratchmark.h` 为准，不将未确认的研究提案写成既定方案。
- 引用样本结论时，注明版本、代码入口及证据局限；外部行为观察结果存在混淆与还原误差。
- Git 远端使用 SSH：`git@github.com:GuitaristRin/Scratchmark.git`。
- `Refs/` 是被 Git 忽略的本地参考目录，不将其中的样本或工具加入版本控制。
