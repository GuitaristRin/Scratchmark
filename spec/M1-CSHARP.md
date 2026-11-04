# M1 C# 实验适配契约

状态：实验规格，依据已确认的 `docs/IMPLEMENTATION_M1.md`。不改变 M0 ABI 或快照格式，也不承诺长期托管 API 兼容。

## 加载与类型

- 本轮仅 Windows x64、.NET 9，加载明确绝对路径的 SDK DLL；进程内固定同一个库，不卸载、不切换版本。创建文档前核对 ABI 1。MSVC、UWP、.NET Native 与其他 CPU 未验证。
- C 类型仍以 `include/scratchmark.h` 为真源，C# 顺序布局、自然对齐、调用约定 Cdecl。SmHandle 与 revision 使用 ulong，长度与状态使用 uint；样本数组只在单次调用期间固定。
- 数值 ID 不能收窄为 IntPtr。文档门面以 ulong 持有句柄，实现 IDisposable 与 finalizer 兜底；显式 Dispose 为正常释放途径。不把 ID 或 revision 当可持久化标识。

## 文档行为

- 每个实例用同一把托管锁串行化全部调用与释放，不向外公开句柄；不同实例可独立使用。Dispose 可重复执行，释放后操作抛 ObjectDisposedException。
- 方法覆盖 Begin / Append / Finish / Cancel / Undo / Redo、GetState / GetStroke / ReadPoints、Save / Load。坐标、活动笔划与快照语义均沿用 M0。
- 调用方显式传 revision；STALE_REVISION 抛含原始状态码的 ScratchmarkException，由调用方重新获取状态。不在适配层无限重试或返回混合数据。
- 非 OK 状态抛 ScratchmarkException，保留 M0 状态码。收到 INTERNAL 时先停用该实例并尝试销毁，再抛原错误。OOM / abort 不能由此隔离。
- Save 在锁内执行容量查询和实际写入，分配上限 64 MiB；Load 接受只读字节跨度，验证失败仍保持原文档。无文件 I/O，文件事务由宿主负责。

## 宿主验收

- 无第三方 NuGet 包，独立 SDK 包提供托管源码及控制台验收项目，宿主不依赖 Rust 源码或编译器。
- 核对三个结构体的大小、所有字段偏移；覆盖完整编辑闭环、失效句柄、批次原子性、revision、容量不足不写数据、损坏 / 不支持快照、重复释放和释放后访问。
- `spec/fixtures/simple-v1.hex` 是独立只读真值。SDK 内只打包这一共享源文件的副本，不在适配工程维护第二套夹具。
- 从夹具读取后导出必须逐字节一致；正常编辑产生的快照与独立 C 宿主结果比较。此为互操作证据，不构成界面、手感、性能或 Android 验证。
