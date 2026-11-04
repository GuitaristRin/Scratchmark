# M1 接口审查：M0 C ABI 对 Windows C# / Android Kotlin 的适配性

状态：Draft（审查文档，不改变任何既定规格与裁定）。日期：2026-11-03。

审查对象与真源：[spec/M0.md](../spec/M0.md)、[include/scratchmark.h](../include/scratchmark.h)、[DECISIONS_2026-11-03.md](DECISIONS_2026-11-03.md) D1/D2、[STATE_2026-11-03.md](STATE_2026-11-03.md)，以及 `crates/scratchmark-core`、`crates/scratchmark-ffi` 现有实现。审查基线：分支 `oc/sm-iface-review`，提交 `800fe51`。

本文件只做接口适配性审查：不改代码、不改 ABI、不改 `spec/M0.md` 与头文件；文中「建议」不构成裁定。工作台通用规则文档在当前借用机不可读（`~/projects/` 与 `~/projects/Arcturius/` 均不存在），本审查按本仓 AGENTS.md 执行。

## 0. 总结论

**M0 的 C ABI 可以被 Windows C# 与 Android Kotlin 两端直接调用，无需修改任何现有签名、结构体或错误码；两端各需一个薄适配层（宿主侧代码），引擎侧零改动。** 设计上最有利于 GC 语言的三点：数值句柄不复用（无 ABA 问题）、不回调宿主且不保留缓冲区指针（pin 只需覆盖单次调用）、状态码而非异常跨界（无 unwind 穿 ABI 风险，`spec/M0.md:22,29,30`）。主要适配工作量在：句柄生命周期与销毁时序的宿主纪律、Kotlin 侧直接缓冲的对齐防御、宿主文件事务，以及一个尚不存在的 JNI 桥。本轮不发现任何「必须改 ABI」项。

## 1. 三类清单

### 1.1 可直接沿用（引擎与 ABI 均不动）

| 项 | 证据 |
| --- | --- |
| 数值句柄 `uint64_t`、0 无效、不复用、destroy 后返回 INVALID_HANDLE | `spec/M0.md:21`；`include/scratchmark.h:15,41`；`crates/scratchmark-ffi/src/lib.rs:27-45`（id 只增不减）；`crates/scratchmark-ffi/src/exports/tests.rs:44-49` |
| 每文档互斥、同句柄串行、不同引擎独立 | `spec/M0.md:21`；`crates/scratchmark-ffi/src/lib.rs:11,47-58`（注册表锁只做查找） |
| 不回调宿主、不拥有窗口或线程、不保留调用方缓冲区指针 | `spec/M0.md:22`；`include/scratchmark.h:43,51-53`（缓冲区均为调用期参数） |
| 状态码错误模型（OK=0…INTERNAL=9），异常不穿 ABI | `spec/M0.md:29,30`；`include/scratchmark.h:16-22`；`crates/scratchmark-core/src/lib.rs:11-23`（`#[repr(u32)]` 一致） |
| 全部结构体为纯标量 POD，自然对齐，两端目标平台均小端 | `include/scratchmark.h:23-37`；布局实测见 §2.4 |
| revision 门控读取，STALE_REVISION 且不写混合/部分输出 | `spec/M0.md:18,20`；`crates/scratchmark-core/src/document.rs:125-138`；`crates/scratchmark-ffi/src/exports/tests.rs:53-66` |
| append 整批先验证、失败不接受任何点（含空批次合法） | `spec/M0.md:12`；`crates/scratchmark-core/src/document.rs:55-78` |
| `sm_abi_version()` / 快照格式版本作为加载期门 | `spec/M0.md:3`；`include/scratchmark.h:39`；`crates/scratchmark-ffi/src/lib.rs:72-75`；`crates/scratchmark-core/src/codec.rs:10,52-54` |
| 64 MiB 快照上限与分配前长度预检、无头部预分配 | `spec/M0.md:37`；`crates/scratchmark-core/src/lib.rs:9`；`crates/scratchmark-core/src/codec.rs:45-73` |
| 资源上限（65,536 批 / 1,000,000 点 / 100,000 笔划）适配两端均为单用户编辑场景，不构成瓶颈语义 | `spec/M0.md:11`；`crates/scratchmark-core/src/lib.rs:6-8` |

### 1.2 需要薄适配层（宿主侧新增代码，不改引擎）

1. **C#**：`SafeHandle` 派生类包装句柄 + P/Invoke 声明 + `STALE_REVISION` 重试助手 + INTERNAL 销毁策略（§2.1、§2.5）。
2. **Kotlin**：`AutoCloseable` 包装 + 小型 JNI 桥（头文件无 `Java_*` 符号）+ 直接缓冲对齐检查与回退（§2.3、§2.7）。
3. **两端**：销毁时序纪律（「destroy 前已进入的操作可能完成，宿主应自行协调」`spec/M0.md:21`）封装进适配层，调用方不感知。
4. **两端**：文件事务（临时文件 + 原子替换 + 保留上一份有效文件）完全在宿主（`spec/M0.md:38`）。
5. **两端**：加载期版本门（ABI 1 + 快照格式 1）与「版本不匹配即拒绝接入」的宿主策略（`spec/M0.md:3`）。

### 1.3 必须改 ABI（本轮不建议，均未实施）

**空清单。** 审查中考察过、但确认可用现有接口或宿主侧手段覆盖的候选，仅记录理由：

- 零复制输入：`spec/M0.md:26` 明确「没有零复制承诺」，且单批至多 65,536 点 × 32 字节 = 2 MiB，复制成本可由宿主分批摊薄。
- 长批次取消/进度回调：与「不回调宿主」原则（`spec/M0.md:22`）冲突，M0 单批验证为 O(n)（`crates/scratchmark-core/src/document.rs:60-66`），当前规模无需中断。
- 异步句柄 / 完成端口：两端都可用宿主线程池 + 同步调用模拟，同步接口反而简化 GC 语言集成。
- `reserved` 字段语义化、句柄指针化：`reserved` 已按全零写出（`crates/scratchmark-core/src/document.rs:40,152`），数值句柄正是 SafeHandle/AutoCloseable 的适配甜点，无改动动机。

## 2. 逐项审查

### 2.1 句柄与生命周期

**引擎侧事实**：句柄是进程内注册表分配的不复用 `uint64_t`，0 无效（`spec/M0.md:21`；`crates/scratchmark-ffi/src/lib.rs:16-35`：`next` 从 1 起且只增，`destroy` 只从映射移除、不回收 id）。destroy 后任何操作返回 INVALID_HANDLE（`crates/scratchmark-ffi/src/lib.rs:37-45`；实测 `crates/scratchmark-ffi/src/exports/tests.rs:44-47`）。并发 destroy 与在途调用是内存安全的：`with_doc` 先克隆 `Arc` 再释放注册表锁（`crates/scratchmark-ffi/src/lib.rs:49-56`），在途操作最多「在逻辑销毁后成功完成」，与 `spec/M0.md:21` 一致；不存在 use-after-free。

**C# 适配**：句柄语义（0 无效）与 `SafeHandleZeroOrMinusOneIsInvalid` 吻合；派生类以 `IntPtr` 承载 64 位 id，`ReleaseHandle` 调 `sm_destroy`，对返回 INVALID_HANDLE（双销毁竞态）应静默返回 true，不得在 finalizer 路径抛异常。SafeHandle 的 P/Invoke 引用计数保护的是宿主侧句柄数值不被 GC 误收，**不**等同于引擎侧销毁协调——适配层仍需保证 destroy 与该句柄上其他调用串行（单一所有者线程或适配层内每句柄锁）。注意 finalizer 不及时：文档内存（至多 1,000,000 点 × 32 字节 ≈ 32 MiB，`crates/scratchmark-core/src/lib.rs:7`）存活到 destroy，长生命周期文档必须显式 Dispose。

**Kotlin 适配**：`long` 承载句柄 + `AutoCloseable`，配合 `use {}`；JVM finalization 已弃用，`Cleaner` 只可作兜底，正确性依赖显式 close。句柄不得格式化、拼接或持久化——它仅在进程内且仅在该动态库实例内有效（注册表是进程级静态，`crates/scratchmark-ffi/src/lib.rs:16`）；宿主在持有活动句柄期间不得卸载动态库（`dlclose` / `FreeLibrary`），适配层应阻止此操作。

### 2.2 线程与重入

**引擎侧事实**：同句柄串行由每文档 `Mutex` 提供，不同文档互不阻塞（`spec/M0.md:21`；`crates/scratchmark-ffi/src/lib.rs:11,47-58`）；同句柄并发 append 不丢点已实测（`crates/scratchmark-ffi/src/exports/tests.rs:91-111`，4 线程 × 100 点全保留）。无回调、不拥有线程（`spec/M0.md:22`），不存在引擎反向进入宿主的线程亲和问题，也不会与宿主锁形成锁序反转。

**STALE_REVISION 协议**：读取必须携带取状态时的 revision，不匹配返回 STALE_REVISION 且不产生混合状态、不写输出（`spec/M0.md:20`；`crates/scratchmark-core/src/document.rs:125-138`；`out_written` 不被写的实测 `crates/scratchmark-ffi/src/exports/tests.rs:53-66`）。revision 从 1 起单调递增，load 也递增且不复用旧值（`spec/M0.md:17,35`；`crates/scratchmark-core/src/document.rs:19,25-30,176-181`）——因此 revision 是**引擎实例内**的计数，宿主不得跨会话持久化或跨引擎比较。

**两端边界建议**：任何线程都可调用，无 UI 线程约束。建议 C#：`append` 批次放输入/计算线程，UI 线程只做 `get_state` 与 begin/finish/cancel/undo/redo 等小操作；渲染预览用「`get_state` 取 revision 快照 → 后台 `sm_get_stroke`/`sm_read_points` → STALE_REVISION 则丢弃重取」模式，无需宿主侧加锁。Kotlin 同构：JNI 调用从主线程做小操作，书写线程做批量 append；注意互斥只保证不丢不撕裂，不定义多次调用的先后顺序，逻辑顺序仍由宿主保证。活动笔划占据读取索引 `stroke_count`（`spec/M0.md:18`；`crates/scratchmark-core/src/document.rs:131-136`），预览循环可直接读它。

### 2.3 缓冲区与内存

**引擎侧事实**：所有输出缓冲区宿主分配/释放，输入被复制进引擎（`spec/M0.md:26`；复制点 `crates/scratchmark-core/src/document.rs:70-77`）。引擎检查 NULL/对齐/容量：`required()` 查 NULL 与 `align_of::<T>()`（`crates/scratchmark-ffi/src/buffers.rs:7-13`），容量与 `isize` 溢出在指针检查之前（`crates/scratchmark-ffi/src/buffers.rs:15-24`），空批次允许 NULL（`spec/M0.md:28`；`crates/scratchmark-ffi/src/exports/tests.rs:39`），错位指针返回 INVALID_ARGUMENT（`crates/scratchmark-ffi/src/exports/tests.rs:41-42`）。输出需互不重叠、调用期独占可写（`spec/M0.md:27`；`crates/scratchmark-ffi/src/exports.rs:65,88`）；标量输出指针必须有效，错误时不写入，save 的 BUFFER_TOO_SMALL 除外（`spec/M0.md:28`；`crates/scratchmark-ffi/src/exports.rs:100-109`）。注意一个错误码顺序细节：count 超限先于 NULL 检查，`sm_append(NULL, u32::MAX)` 返回 LIMIT_EXCEEDED 而非 INVALID_ARGUMENT（`crates/scratchmark-ffi/src/buffers.rs:15-22`；`crates/scratchmark-ffi/src/exports/tests.rs:38`），适配层的错误文案不要假设细分顺序。

**C# 路径**：`SmSample` 全 blittable，P/Invoke 可直接传 `SmSample[]`（封送器自动 pin 调用期）或 `fixed` 下的 `Span<SmSample>`/`Span<byte>`；因为引擎不保留指针（`spec/M0.md:22`），pin 覆盖单次调用即可，无需 `GCHandle` 长期固定。`out_written`/`out_required` 映射 `out uint`。`sm_save` 的「NULL/0 查大小 → 分配 → 重调」两段式直接映射为 C# 的 `stackalloc`/数组两段模式（`spec/M0.md:34`；实测 `crates/scratchmark-ffi/src/exports/tests.rs:68-83`）。

**Kotlin/JNI 路径**：推荐 direct `ByteBuffer`：`GetDirectBufferAddress` 取地址传入，`read_points` 结果可在 Java 侧零额外复制地按偏移读取。**防御点**：JNI 规范不保证 direct buffer 地址按 8 字节对齐（Android 实践通常 16 字节对齐但无规范保证），而引擎要求 `SmSample` 按 8 对齐（`crates/scratchmark-ffi/src/buffers.rs:8`；`include/scratchmark.h:23-28` 含 `double/uint64_t`）——桥层必须检查 `address % 8 == 0`，不满足则退回对齐的 native 暂存缓冲复制。JVM 堆数组（`byte[]`/`long[]`）地址不固定，必须经 `GetPrimitiveArrayCritical`（短临界区）或复制；由于引擎本就复制输入且无零复制承诺（`spec/M0.md:26`），复制路径无语义损失。`SmSample` 步长 32 字节（§2.4），按笔复用一块直接缓冲即可。

### 2.4 ABI 细节

**SM_CALL 与调用约定**：`SM_CALL` 在 `_WIN32` 下为 `__cdecl`，其余为空（`include/scratchmark.h:5-9`）。Windows x64 与 ARM64 只有单一调用约定，`__cdecl` 被编译器接受且等效于缺省——该宏仅在 32 位 x86 有实质区分；C# P/Invoke 缺省 `CallingConvention.Winapi` 在 x64/ARM64 上即与之匹配。当前产物是 `x86_64-pc-windows-gnu`（`tools/build-sdk.ps1:9-10` 明确限定），GNU 工具链在该目标使用微软 x64 ABI，调用层面与 MSVC 宿主兼容；导入表已确认不引入额外 MinGW 运行库 DLL（`docs/STATE_2026-11-03.md:26`），C# 加载不需要附带运行库。

**结构体布局（本机静态断言实测）**：用 GCC 16.2.1（Fedora 44，SysV x86-64）对 `include/scratchmark.h` 以 C11 与 C++17 分别编译 `_Static_assert`/`static_assert` 全部 `sizeof`/`offsetof`/`alignof`，全部通过：

| 结构体 | size | align | 偏移 |
| --- | --- | --- | --- |
| `SmSample`（h:23-28） | 32 | 8 | x=0, y=8, time_us=16, pressure=24, flags=28（尾部 4 字节填充） |
| `SmState`（h:29-33） | 32 | 8 | revision=0, stroke_count=8, active_points=12, has_active=16, can_undo=20, can_redo=24, reserved=28 |
| `SmStrokeInfo`（h:34-37） | 56 | 8 | width=0, min_x=8, min_y=16, max_x=24, max_y=32, color=40, point_count=44, active=48, reserved=52 |

这三个结构体是纯标量 POD，自然对齐下 Windows x64（MSVC 与 GNU 一致）布局规则与 SysV 相同——此为静态推断，未在 Windows/.NET 运行时实测（§4）。Rust 侧 `#[repr(C)]` 与之对应（`crates/scratchmark-core/src/lib.rs:27,52,63`）。

**C# 映射注意**：`[StructLayout(LayoutKind.Sequential)]`、保持字段声明顺序与头文件逐字段一致、缺省 Pack（自然对齐），**不要**设 `Pack=1`；`reserved` 字段必须保留占位（引擎写出全零：`crates/scratchmark-core/src/document.rs:40,152`，宿主应忽略其值）。`SmHandle`→`ulong`（或 SafeHandle 内 `IntPtr`）、`revision`→`ulong`；状态码建议 `enum : uint` 或 const（头文件匿名枚举 `include/scratchmark.h:17-22`）。头文件已带 `extern "C"` 守卫（h:10-12,58-60），C++ 宿主可直接包含（本机 C++17 编译通过）。

**Kotlin/JNI 映射**：无按值结构体跨越 JNI 的需求，全部用偏移常量读取直接缓冲（上表偏移即常量表）；两端目标平台（Windows x64/ARM64、Android ARM64）均为小端，与快照格式小端（`spec/M0.md:36`）一致，无需字节交换，但宿主直接解析格式时应显式声明 LE（C# `BinaryPrimitives`、Kotlin `ByteBuffer.order(LITTLE_ENDIAN)`）。

### 2.5 错误与 panic

**引擎侧事实**：panic 在 ABI 层被 `catch_unwind` 截获并返回 INTERNAL，payload 被 `forget` 以避免二次 panic（`spec/M0.md:30`；`crates/scratchmark-ffi/src/lib.rs:60-70`）；`panic = "unwind"` 是该机制前提（`Cargo.toml:15`）。实测链路：panic 后该文档互斥量中毒，该句柄后续所有操作返回 INTERNAL，destroy 仍成功（`crates/scratchmark-ffi/src/exports/tests.rs:114-122`；中毒路径 `crates/scratchmark-ffi/src/lib.rs:56`）。隔离范围是单文档：每文档独立锁，注册表锁与操作锁分离（`crates/scratchmark-ffi/src/lib.rs:47-58`），一个句柄的 panic 不影响其他句柄。

**宿主销毁策略**：适配层应把「收到 INTERNAL ⇒ 停用该句柄并 destroy」固化为默认策略（对应 `spec/M0.md:30` 「宿主应销毁」）；C# 可在 INTERNAL 后直接 `Dispose()`（SafeHandle finalization 兜底），Kotlin 在 `close()` 中完成。不要因单个 INTERNAL 整进程退出——隔离边界是单文档；但也不要把动态库当作故障隔离边界：OOM、abort 与非法宿主指针不受 `catch_unwind` 保护（`spec/M0.md:30`）。错误映射风格（C# 抛异常 vs 保留状态码、Kotlin 密封结果类）属适配层设计自由，不影响 ABI。

### 2.6 快照与持久化

**引擎侧事实**：save 仅在无活动笔划时可用，仅含已提交笔划，不含撤销/重做历史与 revision（`spec/M0.md:34`；`crates/scratchmark-core/src/codec.rs:6-25`）；NULL/0 可查询所需大小，BUFFER_TOO_SMALL 时写 required（`spec/M0.md:34`；`crates/scratchmark-ffi/src/exports.rs:90-112`）。load 仅在空闲时可用，整份验证成功后才替换文档，失败保持原文档（`spec/M0.md:35`；`crates/scratchmark-core/src/document.rs:173-182`：先 `decode` 后 `edit`）。拒绝列表覆盖空笔划、非有限数、非法 flags/压力、时间逆序、截断、尾随字节与超限（`spec/M0.md:37`；`crates/scratchmark-core/src/codec.rs:49-95`）；64 MiB 上限与逐层预检（总长 → 笔划数 → 每笔点数 → 剩余字节）在分配前完成，不按头部声明预分配（`spec/M0.md:37`；`crates/scratchmark-core/src/codec.rs:45-73`）。

**宿主文件事务**：引擎完全无文件 I/O（`crates/scratchmark-core/src/lib.rs:1`；接口只有内存缓冲，`include/scratchmark.h:54-56`），持久存储的原子替换与保留上一份有效文件是宿主责任（`spec/M0.md:38`）。建议：C# 用临时文件 + `File.Replace`；Kotlin/Android 用临时文件 + `fsync` + 原子 rename，均保留上一份有效副本。快照无完整性校验/加密/长期兼容承诺（`spec/M0.md:38`），需要传输级校验的宿主可在文件层自行附加校验和（SDK 打包已示范 SHA-256 清单做法，`tools/build-sdk.ps1` 末段）。版本门：宿主加载时校验 `sm_abi_version()==1` 且快照头格式版本为 1，不匹配走 UNSUPPORTED_VERSION=7 拒绝（`spec/M0.md:3`；`crates/scratchmark-core/src/codec.rs:10,52-54`）。`load` 后 revision 继续递增而非重置（`crates/scratchmark-core/src/document.rs:176-181`），宿主不得缓存跨会话 revision。

### 2.7 Android 前置（本机全部不可验证，见 §4）

- **现状缺口**：本机无 Android SDK/NDK，Rust 仅安装 Windows GNU target，未生成/运行过任何 Android SO/JNI/AAR（`docs/STATE_2026-11-03.md:36`）。本节为设计核对，不构成已验证结论。
- **目标三元组**：`aarch64-linux-android`（arm64-v8a，首要）、`x86_64-linux-android`（模拟器）、`thumbv7a-linux-androideabi`（armeabi-v7a，按需）。多输入与手机/平板优先是既定产品要求（`docs/PROJECT_BRIEF.md:15-16`），arm64 应为首发验证目标。
- **打包方式**：`crate-type` 已含 `cdylib`（`crates/scratchmark-ffi/Cargo.toml:10`），产物 `libscratchmark.so` 放入 `src/main/jniLibs/<abi>/` 即可被 `System.loadLibrary("scratchmark")` 加载；AGP prefab 是可选的更规范分发形态。SDK 构建脚本当前硬性限定 windows-gnu（`tools/build-sdk.ps1:9-10`），Android 打包脚本需另写，不能复用其目标检查。
- **JNI 符号缺口**：头文件只有 `sm_*` C 符号，无 `Java_*` 导出也无 `JNI_OnLoad`（`include/scratchmark.h:39-56`）——Kotlin 无法直接 `external fun` 绑定指针参数。**最小适配层形状**：一个几十行的 C（或独立 Rust `cdylib`）JNI 桥，导出 `Java_*` 函数：桥持有 `jlong handle` 与 direct buffer 地址，转发状态码；输入从 `jbyteArray`/直接缓冲取址后调 `sm_append`，输出按 §2.3 对齐规则取址调 `sm_read_points`/`sm_save`。静态库 `staticlib`（`crates/scratchmark-ffi/Cargo.toml:10`）可改为把引擎静态链进桥 so，减少 SO 数量，二选一。
- **16 KB 页对齐**：2025-11 起 Play/新设备要求 arm64 16 KB 页；用较新 NDK 与 rustc 默认产物应满足，未验证。
- **panic=unwind 在 Android ABI 上的截获行为**、跨 JNI 调用延迟与功耗均未测量（`docs/STATE_2026-11-03.md:40`）。

## 3. 最小适配层形状（建议，不构成裁定）

```
C#（Windows）                                 Kotlin/Java（Android）
SmEngineHandle : SafeHandle                   ScratchmarkEngine : AutoCloseable
  ReleaseHandle -> sm_destroy                   private external fun …（JNI 桥实现）
  每 P/Invoke：Winapi 约定、 blittable 结构体     handle: Long + 直接缓冲池
SmEngine（门面）                                对齐检查 address % 8 == 0，否则暂存复制
  Begin/Append/Finish/Cancel/Undo/Redo          close() -> sm_destroy（use{} 保证）
  GetState/GetStroke/ReadPoints                 版本门：loadLibrary 后查 sm_abi_version()==1
  Save/Load（byte[] 两段式）                    文件事务：temp + fsync + rename
共享横切（两端同构）：
  版本门（ABI 1 + 格式 1）｜STALE_REVISION 重试助手｜INTERNAL -> 停用并销毁
  销毁与其他调用串行的每句柄纪律｜文件事务｜不持久化句柄与 revision
```

两端适配层均为纯宿主侧新增代码（新目录/新模块），不触碰 `crates/**` 与 ABI。

## 4. 未验证 / 不可在本机验证清单

**本机已实测**：结构体布局（GCC 16.2.1，C11 + C++17 静态断言，SysV x86-64，见 §2.4）；工作区测试套件在本审查基线（`800fe51`）上于 Fedora 44 宿主复跑通过（cargo/rustc 1.99.1，满足 `Cargo.toml:8` 的 rust-version 1.85；借用机容器镜像缺失，因仓库零第三方依赖改用宿主工具链，未安装任何组件）：核心 10 项、FFI 4 项，0 失败 0 忽略，与 `docs/STATE_2026-11-03.md:20` 的 Windows GNU 结果一致，为仓库此前仅有 Windows 证据的行为（并发不丢点、STALE_REVISION 不写输出、panic 截获与中毒销毁、缓冲区边界，`crates/scratchmark-ffi/src/exports/tests.rs:29-122`）补充了 Linux 侧运行证据。

**静态推断、未运行验证**：

1. Windows x64 MSVC/CLR 对三个结构体的封送布局与 SysV 一致（POD 自然对齐下应相同，未在 .NET 运行时实测）。
2. C# P/Invoke 的 `SmSample[]` 自动 pin 与 `SafeHandle` 集成细节（未写 C# 代码验证）。
3. Windows x64 与 ARM64 上 `__cdecl` 等效于缺省调用约定（平台规范，未在 Windows 实测）。

**本机不可验证（缺 Android SDK/NDK，`docs/STATE_2026-11-03.md:36`）**：

4. Android 三元组构建、`libscratchmark.so` 产物与 AAR/jniLibs 打包。
5. `System.loadLibrary` 符号解析、JNI 桥与 `GetDirectBufferAddress` 实际对齐行为。
6. 16 KB 页对齐合规、Android 上 `panic=unwind` 截获、真机延迟/功耗（`docs/STATE_2026-11-03.md:40`）。
7. Windows MSVC/UWP/.NET Native 与 Linux SO 路径（`docs/STATE_2026-11-03.md:37`，Windows C 宿主证据不能外推）。

以上任何一项都不应由本轮文档结论代言；接入时应按 D1 的原则分别记录计算验证与实机验证（`docs/DECISIONS_2026-11-03.md:11`）。

## 5. 与 D1/D2 边界的一致性

本审查未把渲染、窗口或手感划进引擎：接口输出的几何语义是「中心线 + 宽度 + 保守边界，光栅化由前端完成」（`spec/M0.md:8`），且压力、平滑、预测、呈现均明确不在本阶段验收范围（`spec/M0.md:10,42`）。D1 的纯引擎定位（`docs/DECISIONS_2026-11-03.md:9-11`）与 D2 的 Rust 核心 + C#/Kotlin 原生适配方向（`docs/DECISIONS_2026-11-03.md:17`）同本结论一致：薄适配层全部位于前端侧，引擎仍承担笔迹计算、编辑与数据处理实体工作，不是纯接口声明。STATE 提出的「避免把实验几何输出误当最终渲染契约」（`docs/STATE_2026-11-03.md:45`）在 §2.4/§2.6 中以「宿主必须固定匹配版本 + reserved 忽略 + revision 不持久化」落成可执行约束。
