# 发布前清单（0.1.0-pre）

状态：**0.1.0-pre 已发布**（见 GitHub Releases）。本文件记录公开发布前必须先决定或完成的事；做完一项勾一项，全部完成才动
`git push` / `gh repo edit --visibility public` / `gh release create`。

## 一、必须先决定

- [x] **许可证**：见下节方案，选定后写入 `LICENSE`（与 `package.json`/`Cargo.toml`/nuspec 的 `license` 字段保持一致）。
- [x] **历史处理**：仓库现有 127 个提交，其中多轮文档提到了参考产品的**反向工程结论**。仅删除工作区文件不够——
      `git log -S` 仍能查到。可选：
  - **A. 重写历史**：删除相关文件并清理提交信息，然后 `git push --force`。工程历史保留，但所有 SHA 变化。
  - **B. 压成单个提交**：用当前（已清理的）工作区做一次 `0.1.0-pre` 提交，`--force` 覆盖 `main`。公开历史从 1 个提交开始。
  - **C. 新开公开仓库**：本仓继续私有开发，另建公开仓只放发布快照。
- [x] **制品范围**：pre-release 是否附二进制（Windows `.zip`、`.nupkg`、`wasm`、Android `.so`、Linux `.so`）？
      附制品要同时满足依赖许可证（当前唯一第三方依赖 `jni` 是 `MIT/Apache-2.0`，选 Apache 需附 `NOTICE`）。

## 二、清理范围（当前工作区）

已跟踪文件中提到参考产品 / 反向工程 / 算法来源的有 **33 个**，按处理方式分三类：

| 处理 | 文件 | 说明 |
| --- | --- | --- |
| 直接删除 | `docs/REFERENCE_FINDINGS.md`、`docs/research/2026-11-07-eraser-and-paint-engine.md`、`docs/research/2026-11-07-oppo-capability-inventory.md` | 整篇都是参考产品的结论 |
| 重写表述 | `spec/M11`–`M17`、`docs/IMPLEMENTATION_M6`–`M17`、`docs/STATE_2026-11-06/07`、`docs/DECISIONS`（`D15`/`D17`/`D18`/`D22`…）、`docs/PROPOSALS`、`docs/PROJECT_BRIEF`、`docs/README`、`docs/VERIFICATION`、`AGENTS.md`、`README.md` | 把"因为某产品怎么做，所以我们这样做"改成"为与主流移动便签行为一致，需求如下"——**保留需求，去掉出处** |
| 检查确认 | `docs/SDK_README.md`、`docs/PLATFORMS.md`、`docs/API.md`、`docs/ARCHITECTURE.md` | 各一处提法，改成中性表述 |

原则：**需求照写，来源不写**。规格仍然要说清"倾斜要在 `[0, π/2]`"这类可验收的约束，但不再写它从哪来。

## 三、许可证方案（待你选）

| 方案 | 适合 | 代价 |
| --- | --- | --- |
| **MIT** | 想让它被最容易地嵌入（含闭源商业 App） | 无 copyleft；他人可闭源分发你的代码 |
| **Apache-2.0** | 同上 + 明确的专利授权与贡献条款 | 需要 `NOTICE` 文件、改动声明；文本更长 |
| **MIT + Apache-2.0 双许可** | Rust 生态事实标准，嵌入方自选 | 两份文本都要维护 |
| **MPL-2.0** | 想允许链接/嵌入，但**改动的文件**必须开源 | 文件级 copyleft，嵌入方需理解边界 |
| **LGPL-3.0** | 想让库保持开源但允许商业 App 使用 | 移动端静态链接要提供可重新链接手段，实务成本高 |
| **AGPL-3.0** | 防止任何人闭源提供服务 | 移动 App 分发也会触发，商业采用门槛很高 |
| **BSL / Elastic-2.0** | 源码可见、限制竞争性商用，若干年后转开源 | 不属于 OSI 开源；"public repo" 但不等于开源 |
| **不分许可证（All rights reserved）** | 只公开代码供阅读 | 法律上禁止他人使用；公开仓库容易让人误以为可用 |

倾向意见：这是一个**要被嵌进 App 的引擎**，采用门槛越低越好 → **MIT 或 MIT/Apache-2.0 双许可**。
若担心被同行直接拿去闭源卖，再考虑 MPL-2.0；AGPL 与移动 App 的商业模式基本冲突。

## 四、发布动作（清理完成后）

```powershell
# 1) 版本号：Cargo.toml / 绑定 nuspec / PLATFORMS 文档同步到 0.1.0
# 2) 生成制品并验收
pwsh tools/build-sdk.ps1; pwsh tools/build-wasm.ps1; pwsh tools/build-android.ps1; pwsh tools/build-linux.ps1; pwsh tools/pack-nuget.ps1
# 3) 可见性
gh repo edit GuitaristRin/Scratchmark --visibility public --accept-visibility-change-consequences
# 4) 预发布
gh release create v0.1.0-pre --prerelease --title "0.1.0-pre" --notes-file <notes.md> <制品...>
```

注意：`gh repo edit --visibility public` 会立刻暴露**全部历史**；执行前必须已选定历史处理方式。
