package com.scratchmark

/**
 * JNI 桥：只做类型编组，语义全部在 Rust 侧（`crates/scratchmark-jni`）。
 *
 * 批量数据用 `ByteArray`：
 * - 样本：32 字节/点（x f64, y f64, time u64, pressure f32, flags u32）
 * - 笔刷点：24 字节/点（x f64, y f64, width f64）
 *
 * 状态码与 C ABI 一致：0 = OK，1 = 参数非法，3 = 状态非法，5 = revision 过期，6 = 超限，-1 = 其它失败。
 */
internal object Native {
    init {
        System.loadLibrary("scratchmark_jni")
    }

    @JvmStatic external fun abiVersion(): Int
    @JvmStatic external fun features(): Long

    @JvmStatic external fun create(): Long
    @JvmStatic external fun destroy(handle: Long)

    @JvmStatic external fun begin(handle: Long, width: Double, color: Int): Int
    @JvmStatic external fun append(handle: Long, samples: ByteArray): Int
    @JvmStatic external fun finish(handle: Long): Int
    @JvmStatic external fun cancel(handle: Long): Int
    @JvmStatic external fun undo(handle: Long): Int
    @JvmStatic external fun redo(handle: Long): Int

    /** `[revision, strokeCount, activePoints, hasActive, canUndo, canRedo]` */
    @JvmStatic external fun state(handle: Long): LongArray

    @JvmStatic external fun save(handle: Long): ByteArray?
    @JvmStatic external fun load(handle: Long, payload: ByteArray): Int

    @JvmStatic external fun points(handle: Long, revision: Long, index: Int): ByteArray?
    @JvmStatic external fun brushPoints(
        handle: Long,
        revision: Long,
        index: Int,
        alpha: Double,
        minWidthRatio: Double,
        pressureEnabled: Int,
    ): ByteArray?

    @JvmStatic external fun hitTest(handle: Long, revision: Long, x: Double, y: Double, tolerance: Double): Int
    @JvmStatic external fun eraseStroke(handle: Long, revision: Long, index: Int): Int

    /** `path` 是扁平的 `[x0, y0, x1, y1, …]`。 */
    @JvmStatic external fun erasePath(handle: Long, revision: Long, path: DoubleArray, radius: Double): Int

    /** `[minX, minY, maxX, maxY, strokeCount, found]` */
    @JvmStatic external fun documentBounds(handle: Long, revision: Long): DoubleArray
    @JvmStatic external fun translateAll(handle: Long, revision: Long, dx: Double, dy: Double): Int
    @JvmStatic external fun brushPreset(kind: Int, level: Int): Double

    @JvmStatic external fun copyStrokes(handle: Long, revision: Long, indices: IntArray): ByteArray?
    @JvmStatic external fun cutStrokes(handle: Long, revision: Long, indices: IntArray): ByteArray?
    @JvmStatic external fun paste(handle: Long, revision: Long, payload: ByteArray, dx: Double, dy: Double): Int
}
