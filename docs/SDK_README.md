# Scratchmark M0 开发 SDK

这是无 UI 的实验性原生引擎，SDK 0.1.0 / ABI 1 / 快照格式 1。不提供稳定发布兼容承诺；精确行为参包内 `spec/M0.md`。

## 包内容

- `bin/scratchmark.dll`：Windows x64 GNU 目标的动态库。
- `include/scratchmark.h`：C 接口与类型声明，使用 C 调用约定。
- `examples/smoke.c`：只靠动态加载的独立宿主示例，不链接引擎源码。
- `verify-sdk.ps1`：核对 SHA-256 后编译并运行该示例，需要已安装的 clang。
- `build-info.json`：版本、目标、源码提交及 dirty 标记、编译器身份。
- `SHA256SUMS`：包内原始文件的校验清单，不含清单自身与后续验证产物。

在此目录的 PowerShell 中运行：

```powershell
.\verify-sdk.ps1
```

会创建唯一的 `verification-*` 目录，记录实际读写的快照文件，既有文件不会被覆盖。可复制整个 SDK 目录到没有引擎源码的路径再次运行，不要求 Rust 或 Git submodule。

## 调用顺序

1. 动态加载库，核对 `sm_abi_version()`，创建句柄。
2. `sm_begin` → 一次或多次 `sm_append`；每次输入后均可读取活动笔迹。
3. 用 `sm_get_state` 取得 revision，以同一 revision 读取笔划元数据及分段点；若状态改变，重新取得 revision。
4. `sm_finish` 确认笔划或 `sm_cancel` 丢弃；空闲时可 undo / redo。
5. `sm_save` 查询所需容量，再写入宿主缓冲区；宿主负责写盘和文件事务。
6. 新建引擎后 `sm_load` 恢复，可以继续书写；所有句柄在结束时 destroy。

同一句柄的读取步骤之间若有其他线程修改，可能收到 STALE_REVISION，这是重试信号，不是完整快照已返回。无需多线程时，宿主顺序调用即可。

## 当前能力与限制

支持固定宽度折线、活动笔迹、边界、整笔历史、可编辑快照。没有窗口或绘图后端；输入压力仅记录，不改变线宽。平滑、预测、擦除、套索、页面导航和设备防误触未实现。

此包仅验证 Windows x64 原生 C 动态调用，不代表 UWP / .NET Native、C#、Android JNI / AAR 或 Linux 已验收。SO 必须按目标系统和 CPU 另行构建，不可通过改扩展名获得。静态库可由相同工程构建，本包暂不包含静态链接接入示例。

实际呈现由前端完成；此包不能证明手感或端到端延迟。未进行性能基准测试，不承诺每秒点数或调用开销比例。授权与发布策略仍待确定，本次交付是本地开发验收包。
