# 架构

> **实验版（0.1.0-pre）**：接口与快照格式不承诺兼容，二进制制品仅在本文说明的范围内验证过。
> **已验证**与**明确未验证**的项目见 [验证矩阵](VERIFICATION.md)；未验证项不允许表述为"支持"。

只写当前代码里真实存在的东西；数字取自源码与本机实跑。

## 分层

```
宿主（C / C# / Kotlin / JS）
        │  只通过 C ABI 或 JNI 调用
        ▼
crates/scratchmark-ffi      C ABI（cdylib + staticlib，52 个导出，ABI 1）
crates/scratchmark-jni      JNI（cdylib，24 个导出）
        │  只做编组：句柄表、指针校验、catch_unwind
        ▼
crates/scratchmark-core     全部引擎语义（只依赖 std，无窗口 / 无文件 / 无网络）
```

铁律：**语义只在 core**。FFI 与 JNI 层不实现任何几何、擦除、快照逻辑——否则两个平台会漂移。
`docs/DECISIONS_2026-11-03.md` 的 D1 记的就是这条边界。

## 数据模型

```rust
Sample { x: f64, y: f64, time_us: u64, pressure: f32, flags: u32 }   // 32 字节
SampleAxes { tilt: f32, orientation: f32 }                            // 与 points 等长
Stroke { width: f64, color: u32, points: Vec<Sample>, axes: Vec<SampleAxes>,
         bounds: Option<[f64; 4]>, brush_cache: Option<BrushCache> }
BrushPoint { x: f64, y: f64, width: f64 }                             // 24 字节，派生量
```

`flags` 是**位域**：bit0 = 该点带真实压力，bit1 = 该点是擦除切出来的端点（M17），其余位必须为 0。
写入通道（`sm_append` / `sm_append_axes`）拒绝 bit1——切点标记只能由引擎产生，避免宿主伪造。

### 文档

```rust
Document {
    strokes: Vec<Stroke>,          // 已提交，z 序 = 下标
    active: Option<Stroke>,        // 正在写的那一笔，不在 strokes 里
    undo: Vec<Edit>, redo: Vec<Edit>,
    retained_points: usize, retained_strokes: usize,   // 含历史在内的预算计数
    revision: u64,                 // 每次编辑 +1
    page_id: u64,
}
```

不变量：

- `revision` 单调递增；新建文档从 **1** 开始（不是 0）。所有写接口都要带 `revision`，不匹配返回 `SM_STALE_REVISION`。
- 有活动笔划时，除 `append` / `finish` / `cancel` 与只读查询外，编辑类接口返回 `SM_INVALID_STATE`。
- `retained_points` / `retained_strokes` 统计**存活 + 历史**两份，撤销重做不会让它们下降；超限返回 `SM_LIMIT_EXCEEDED`。
- `axes` 与 `points` 永远等长（`Stroke::extend` 负责补齐默认轴向）。

## 撤销模型

历史条目是四种之一：

| 条目 | 含义 | 谁在用 |
| --- | --- | --- |
| `Add(Option<Stroke>)` | 提交一笔（含被 redo 丢弃时回填） | begin/finish |
| `Erase { index, stroke }` | 删一笔 | 整笔擦除 |
| `Replace(Vec<ReplaceOp>)` | 按下标替换 / 批量删除 / 整份平移 | 局部擦除、变换、删除所选、清空、平移、规整 |
| `Insert { index, count, strokes }` | 整批插入 | 粘贴 |

`ReplaceOp { index, after_count, before, after }` 里的 `index` 是**施加时刻**的下标。批量删除按升序逐个移除时，
每个 op 记录的是移除后的位置（`index - removed`），回滚时按记录逆序插回。
一次 API 调用只产生**一个**条目：整笔擦除批量删除、清空、整份平移、原子剪切都是这个原则。

## 快照

自描述字节，小端；`MAX_SNAPSHOT = 64 MiB`。

| 格式 | 魔数（8B） | 头 | 每笔 | 每点 |
| --- | --- | --- | --- | --- |
| 1 | `SCRATCH\0` | u32 版本=1、u32 笔数 | f64 宽、u32 色、u32 点数 | 32 字节样本 |
| 2 | `SCRATCH2` | 同上 + u64 页面标识 | 同上 | 32 字节样本 |
| 3 | `SCRATCH3` | 同格式 2 | 同上 | 32 字节样本 + f32 倾斜 + f32 朝向 |

写入策略（`Codec::save`）：任一已提交笔划带非默认轴向 → 格式 3；否则有页面标识 → 格式 2；否则格式 1。
读入接受 1/2/3，格式 1/2 载入后轴向为默认值。剪贴板（复制 / 剪切）恒为页面标识 0：带轴向时格式 3，否则格式 1；
粘贴只接受格式 1 或**页面标识为 0 的格式 3**（格式 2 视为“整份文档”，拒绝）。

## 限制与常量

| 常量 | 值 | 说明 |
| --- | --- | --- |
| `MAX_BATCH` | 65,536 | 单次 `append` 的样本上限 |
| `MAX_POINTS` | 1,000,000 | 文档内样本总数（含历史） |
| `MAX_STROKES` | 100,000 | 笔划总数 |
| `MAX_SNAPSHOT` | 64 MiB | 快照与剪贴板字节上限；也是 `sm_alloc` 的单次上限 |
| `MAX_COORD` | 1e9 | 坐标绝对值上限（越界即参数非法） |
| `MAX_PREDICTION_POINTS` | 64 | 一次预测最多给几个点 |
| `BRUSH_LEVELS` | 5 | 笔型档位数 |

## 版本号
公开版本从 **0.1.0** 起；开发期内部曾用更细的编号（如 0.10–0.18），那些编号不再出现于对外产物。
能力请以里程碑（`M0`–`M19`）与能力位为准，不要依赖内部版本号。

## 能力位

`sm_features()` 返回位掩码，宿主用它决定要不要用某个接口。当前用到 bit 0–16：

| 位 | 能力 | 引入 |
| --- | --- | --- |
| 0 | 派生笔刷（平滑 + 逐点笔宽） | M2 |
| 1 | 页面标识与视口变换 | M3 |
| 2 | 命中与整笔擦除 | M4 |
| 3 | 局部擦除（沿路径切分） | M5 |
| 4 | 套索命中与批量删除 | M6 |
| 5 | 选区变换与复制粘贴 | M7 |
| 6 | 形状规整 | M8 |
| 7 | 视口旋转 | M9 |
| 8 | 预测笔迹 | M10 |
| 9 | 逐点轴向 | M11 |
| 10 | 文档边界与整份平移 | M12 |
| 11 | 笔型尺寸预置 | M13 |
| 12 | 点间距重采样 | M14 |
| 13 | 预测刷新率 | M15 |
| 14 | 原子剪切 | M16 |
| 15 | 擦除切点标记 | M17 |
| 16 | 宿主内存分配 | M18 |

## 状态码

| 值 | 名称 | 典型触发 |
| --- | --- | --- |
| 0 | `SM_OK` | 成功 |
| 1 | `SM_INVALID_ARGUMENT` | 指针为空、坐标非有限、宽度 ≤ 0、`flags` 越位、索引重复/越界 |
| 2 | `SM_INVALID_HANDLE` | 句柄不存在或已销毁 |
| 3 | `SM_INVALID_STATE` | 有活动笔划时做编辑、或没有活动笔划时 append |
| 4 | `SM_BUFFER_TOO_SMALL` | 输出区不足；同时回填所需大小 |
| 5 | `SM_STALE_REVISION` | 传入的 revision 与当前不一致 |
| 6 | `SM_INVALID_DATA` | 快照损坏、魔数/字段非法 |
| 7 | `SM_UNSUPPORTED_VERSION` | 已知魔数但版本不匹配 |
| 8 | `SM_LIMIT_EXCEEDED` | 超出上面的常量 |
| 9 | `SM_INTERNAL` | 内部 panic 被 `catch_unwind` 拦住 |

## 线程与所有权

- FFI 与 JNI 各有一张句柄表：`HashMap<u64, Arc<Mutex<Document>>>`，句柄是递增的 `u64`。
  **文档级的锁**保证同一文档的调用串行；不同文档可以并发。
- 引擎不持有宿主内存；除 `sm_alloc` 分配出的缓冲区外，所有指针都只在调用期间有效。
- 引擎不回调、不起线程、不读时钟：`time_us` 完全由宿主给。
- 派生量（笔刷点、预测、重采样）都是**只读**，不进快照、不改 revision；`brush_cache` 是每笔的缓存，`read_brush_points`
  在参数不变时复用它。

## 模块地图

| 文件 | 职责 |
| --- | --- |
| `document.rs` | 文档状态、编辑历史、提交/取消、撤销重做、预算 |
| `codec.rs` | 快照编解码（格式 1/2/3）与剪贴板编码 |
| `brush.rs` | M2 因果平滑与逐点笔宽派生 |
| `erase.rs` | M5 切分几何（二次方程闭式解、包围盒裁剪） |
| `select.rs` | M6 套索命中、批量删除、清空、原子剪切 |
| `transform.rs` | M7 仿射变换、复制粘贴、整份平移 |
| `shape.rs` | M8 直线/矩形/圆/箭头拟合与残差 |
| `viewport.rs` | M3/M9 视口与旋转 |
| `predict.rs` | M10/M15 预测与刷新率窗 |
| `resample.rs` | M14 按笔宽等距取点 |
| `preset.rs` | M13 笔型档位表 |
| `ffi/exports.rs` | C ABI 编组（按里程碑拆成多个 `*_tests.rs`） |
| `jni/lib.rs` | JNI 编组，24 个导出 |

