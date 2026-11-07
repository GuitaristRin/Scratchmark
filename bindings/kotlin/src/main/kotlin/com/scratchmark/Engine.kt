package com.scratchmark

import java.nio.ByteBuffer
import java.nio.ByteOrder

/** 采样点：与引擎的 32 字节布局一一对应。 */
class Sample(
    @JvmField var x: Double,
    @JvmField var y: Double,
    @JvmField var timeUs: Long,
    @JvmField var pressure: Float = 0f,
    @JvmField var flags: Int = 0,
)

/** 笔刷点：坐标 + 该位置的完整笔宽。 */
class BrushPoint(
    @JvmField var x: Double,
    @JvmField var y: Double,
    @JvmField var width: Double,
)

/** 文档状态。 */
class DocumentState(
    @JvmField val revision: Long,
    @JvmField val strokeCount: Int,
    @JvmField val activePoints: Int,
    @JvmField val hasActive: Boolean,
    @JvmField val canUndo: Boolean,
    @JvmField val canRedo: Boolean,
)

/** 文档边界；`found = false` 表示还没有已提交的笔划。 */
class DocumentBounds(
    @JvmField val minX: Double,
    @JvmField val minY: Double,
    @JvmField val maxX: Double,
    @JvmField val maxY: Double,
    @JvmField val strokeCount: Int,
    @JvmField val found: Boolean,
)

/** `SmSample.flags` 的位语义（与 `spec/M17.md` 一致）。 */
object SampleFlags {
    const val PRESSURE = 1
    const val CUT_END = 2
}

/** 笔型尺寸预置（与 `spec/M13.md` 一致）。 */
enum class BrushKind(val code: Int) {
    Pencil(1), Marker(2), Ballpen(3), Pen(4), Eraser(5);
}

/** 引擎失败：`status` 与 C ABI 状态码一致。 */
class ScratchmarkException(val status: Int, message: String) : RuntimeException(message)

/**
 * 文档句柄。用完必须 [close]：原生侧按句柄持有文档，不关闭会泄漏。
 *
 * 线程约定：同一实例不要跨线程并发调用；需要并发时各自开一个 [Engine]。
 */
class Engine : AutoCloseable {
    private var handle: Long = Native.create()

    init {
        if (handle == 0L) throw ScratchmarkException(-1, "sm_create failed")
    }

    /** 当前文档状态。 */
    val state: DocumentState
        get() {
            val values = Native.state(handle)
            return DocumentState(
                revision = values[0],
                strokeCount = values[1].toInt(),
                activePoints = values[2].toInt(),
                hasActive = values[3] != 0L,
                canUndo = values[4] != 0L,
                canRedo = values[5] != 0L,
            )
        }

    fun begin(width: Double, color: Int) = check(Native.begin(handle, width, color))

    /** 追加一批采样点（同一进程内按时间非递减）。 */
    fun append(samples: List<Sample>) = check(Native.append(handle, encodeSamples(samples)))

    fun finish() = check(Native.finish(handle))
    fun cancel() = check(Native.cancel(handle))
    fun undo() = check(Native.undo(handle))
    fun redo() = check(Native.redo(handle))

    /** 快照字节：自描述的，宿主只当二进制保存。 */
    fun save(): ByteArray = Native.save(handle) ?: throw ScratchmarkException(-1, "sm_save failed")

    fun load(payload: ByteArray) = check(Native.load(handle, payload))

    /** 原始样本；`index == strokeCount` 时读活动笔划。 */
    fun points(index: Int, revision: Long = state.revision): List<Sample> {
        val bytes = Native.points(handle, revision, index) ?: return emptyList()
        return decodeSamples(bytes)
    }

    /** 派生笔刷点：逐点完整笔宽，宿主直接拿它画多边形。 */
    fun brushPoints(
        index: Int,
        alpha: Double = 0.5,
        minWidthRatio: Double = 0.25,
        pressureEnabled: Boolean = true,
        revision: Long = state.revision,
    ): List<BrushPoint> {
        val bytes = Native.brushPoints(
            handle, revision, index, alpha, minWidthRatio, if (pressureEnabled) 1 else 0,
        ) ?: return emptyList()
        return decodeBrushPoints(bytes)
    }

    /** 命中测试：返回笔划下标，未命中返回 -1。 */
    fun hitTest(x: Double, y: Double, tolerance: Double = 0.0, revision: Long = state.revision): Int =
        Native.hitTest(handle, revision, x, y, tolerance)

    fun eraseStroke(index: Int, revision: Long = state.revision) =
        check(Native.eraseStroke(handle, revision, index))

    /** 沿折线擦除：`path` 是扁平的 `[x0, y0, x1, y1, …]`。 */
    fun erasePath(path: DoubleArray, radius: Double, revision: Long = state.revision) =
        check(Native.erasePath(handle, revision, path, radius))

    fun bounds(revision: Long = state.revision): DocumentBounds {
        val values = Native.documentBounds(handle, revision)
        return DocumentBounds(values[0], values[1], values[2], values[3], values[4].toInt(), values[5] != 0.0)
    }

    /** 整份平移：一次调用一个撤销条目。 */
    fun translateAll(dx: Double, dy: Double, revision: Long = state.revision) =
        check(Native.translateAll(handle, revision, dx, dy))

    fun copyStrokes(indices: IntArray, revision: Long = state.revision): ByteArray =
        Native.copyStrokes(handle, revision, indices) ?: throw ScratchmarkException(-1, "copy failed")

    /** 原子剪切：产出剪贴板字节并删除选区，只占一个撤销条目。 */
    fun cutStrokes(indices: IntArray, revision: Long = state.revision): ByteArray =
        Native.cutStrokes(handle, revision, indices) ?: throw ScratchmarkException(-1, "cut failed")

    fun paste(payload: ByteArray, dx: Double = 0.0, dy: Double = 0.0, revision: Long = state.revision) =
        check(Native.paste(handle, revision, payload, dx, dy))

    override fun close() {
        if (handle != 0L) {
            Native.destroy(handle)
            handle = 0L
        }
    }

    private fun check(status: Int) {
        if (status != 0) throw ScratchmarkException(status, "scratchmark status $status")
    }

    companion object {
        const val ABI_VERSION = 1

        /** 该构建支持的能力位（bit 0–16）。 */
        fun features(): Long = Native.features()

        /** 笔型档位比例（0, 1]；`level` 从 1 开始。 */
        fun brushPresetRatio(kind: BrushKind, level: Int): Double =
            Native.brushPreset(kind.code, level)

        /** 档位换算成实际线宽；换算出 0 表示参数非法。 */
        fun brushPresetWidth(kind: BrushKind, level: Int, maxWidth: Double): Double =
            brushPresetRatio(kind, level) * maxWidth

        private fun encodeSamples(samples: List<Sample>): ByteArray {
            val buffer = ByteBuffer.allocate(samples.size * 32).order(ByteOrder.LITTLE_ENDIAN)
            for (sample in samples) {
                buffer.putDouble(sample.x)
                buffer.putDouble(sample.y)
                buffer.putLong(sample.timeUs)
                buffer.putFloat(sample.pressure)
                buffer.putInt(sample.flags)
            }
            return buffer.array()
        }

        private fun decodeSamples(bytes: ByteArray): List<Sample> {
            val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
            val samples = ArrayList<Sample>(bytes.size / 32)
            while (buffer.remaining() >= 32) {
                samples.add(
                    Sample(
                        x = buffer.double,
                        y = buffer.double,
                        timeUs = buffer.long,
                        pressure = buffer.float,
                        flags = buffer.int,
                    )
                )
            }
            return samples
        }

        private fun decodeBrushPoints(bytes: ByteArray): List<BrushPoint> {
            val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN)
            val points = ArrayList<BrushPoint>(bytes.size / 24)
            while (buffer.remaining() >= 24) {
                points.add(BrushPoint(x = buffer.double, y = buffer.double, width = buffer.double))
            }
            return points
        }
    }
}
