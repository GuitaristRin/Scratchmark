using System.Runtime.InteropServices;

namespace Scratchmark;

public enum Status : uint
{
    Ok, InvalidArgument, InvalidHandle, InvalidState, BufferTooSmall,
    StaleRevision, InvalidData, UnsupportedVersion, LimitExceeded, Internal
}

[StructLayout(LayoutKind.Sequential)]
public struct Sample
{
    public double X, Y;
    public ulong TimeUs;
    public float Pressure;
    public uint Flags;
}

[StructLayout(LayoutKind.Sequential)]
public struct State
{
    public ulong Revision;
    public uint StrokeCount, ActivePoints, HasActive, CanUndo, CanRedo, Reserved;
}

[StructLayout(LayoutKind.Sequential)]
public struct StrokeInfo
{
    public double Width, MinX, MinY, MaxX, MaxY;
    public uint Color, PointCount, Active, Reserved;
}

public sealed class ScratchmarkException : Exception
{
    public ScratchmarkException(Status status) : base($"Scratchmark: {status}") => Status = status;

    /// <summary>带自定义说明的场景（例如库定位失败），仍携带状态码。</summary>
    public ScratchmarkException(Status status, string message) : base(message) => Status = status;

    public Status Status { get; }
}


public readonly record struct HitResult(bool Found, uint Index);

[StructLayout(LayoutKind.Sequential)]
public struct BrushOptions
{
    public double Alpha, MinWidthRatio;
    public uint PressureEnabled, Reserved;
}

[StructLayout(LayoutKind.Sequential)]
public struct BrushPoint
{
    public double X, Y, Width;
}

/* M3 视口由宿主保存；映射与页面域约束参 spec/M3.md。 */
[StructLayout(LayoutKind.Sequential)]
public struct Viewport
{
    public double Scale, OffsetX, OffsetY;
    public uint Reserved;

    public static Viewport Identity => new() { Scale = 1.0 };

    public readonly Point PageToViewport(Point page)
    {
        Status status = Native.PageToViewport(this, page, out Point output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    public readonly Point ViewportToPage(Point viewport)
    {
        Status status = Native.ViewportToPage(this, viewport, out Point output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    public readonly double WidthPageToViewport(double width)
    {
        Status status = Native.WidthPageToViewport(this, width, out double output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    public readonly double WidthViewportToPage(double width)
    {
        Status status = Native.WidthViewportToPage(this, width, out double output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    public readonly Bounds BoundsPageToViewport(Bounds page)
    {
        Status status = Native.BoundsPageToViewport(this, page, out Bounds output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }
}

[StructLayout(LayoutKind.Sequential)]
public struct Point
{
    public double X, Y;
}

[StructLayout(LayoutKind.Sequential)]
public struct Bounds
{
    public double MinX, MinY, MaxX, MaxY;
}

/* M7 仿射变换：页面坐标到页面坐标；原点、轴线与手柄由宿主决定。 */
[StructLayout(LayoutKind.Sequential)]
public struct Transform
{
    public double M00, M01, M10, M11, Dx, Dy;

    public static Transform Identity => new() { M00 = 1.0, M11 = 1.0 };

    public static Transform Translate(double dx, double dy) => new() { M00 = 1.0, M11 = 1.0, Dx = dx, Dy = dy };

    public static Transform Scale(double sx, double sy) => new() { M00 = sx, M11 = sy };

    public static Transform Rotate(double radians)
    {
        double cos = Math.Cos(radians);
        double sin = Math.Sin(radians);
        return new Transform { M00 = cos, M01 = -sin, M10 = sin, M11 = cos };
    }
}
/* M8 形状规整：种类显式指定，error 是残差（页面单位），是否采纳由宿主决定。 */
public enum ShapeKind : uint
{
    Line = 1, Rectangle = 2, Circle = 3, Arrow = 4
}

[StructLayout(LayoutKind.Sequential)]
public struct ShapeInfo
{
    public uint Kind, StrokeCount, PointCount, Reserved;
    public double Error;
}
/* M9 带旋转的视口：先缩放、再旋转、最后平移；旧 Viewport 等价于 Angle = 0。 */
[StructLayout(LayoutKind.Sequential)]
public struct ViewportEx
{
    public double Scale, Angle, OffsetX, OffsetY;
    public uint Reserved;

    public static ViewportEx Identity => new() { Scale = 1.0 };

    public readonly Point PageToViewport(Point page)
    {
        Status status = Native.PageToViewportEx(this, page, out Point output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    public readonly Point ViewportToPage(Point viewport)
    {
        Status status = Native.ViewportToPageEx(this, viewport, out Point output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    public readonly double WidthPageToViewport(double width)
    {
        Status status = Native.WidthPageToViewportEx(this, width, out double output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    public readonly double WidthViewportToPage(double width)
    {
        Status status = Native.WidthViewportToPageEx(this, width, out double output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    public readonly Bounds BoundsPageToViewport(Bounds page)
    {
        Status status = Native.BoundsPageToViewportEx(this, page, out Bounds output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    /// <summary>以视口点为锚缩放：锚点下的页面点保持不变。</summary>
    public readonly ViewportEx ZoomAt(Point anchor, double factor)
    {
        Status status = Native.ViewportExZoomAt(this, anchor, factor, out ViewportEx output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }

    /// <summary>把页面边界适配进视口矩形，带旋转与内边距。</summary>
    public static ViewportEx Fit(Bounds page, double viewWidth, double viewHeight, double angle, double padding)
    {
        Status status = Native.ViewportExFit(page, viewWidth, viewHeight, angle, padding, out ViewportEx output);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return output;
    }
}
/* M10 预测笔迹：只读外推，复用 M2 笔刷配置，输出沿用 BrushPoint。 */
[StructLayout(LayoutKind.Sequential)]
public struct PredictionOptions
{
    public double HorizonUs, MaxDistance;
    public uint Count, Reserved;
    public BrushOptions Brush;
}
/* M11 逐点轴向：倾斜 [0, π/2]，朝向 [-π, π]，单位弧度；0 表示垂直或未提供。 */
[StructLayout(LayoutKind.Sequential)]
public struct SampleAxes
{
    public float Tilt, Orientation;
}
/* M13 笔型尺寸预置：五档比例；纹理与透明度仍由宿主决定。 */
public enum BrushKind : uint
{
    Pencil = 1, Marker = 2, Ballpen = 3, Pen = 4, Eraser = 5
}

public static class BrushPresets
{
    public const uint Levels = 5;

    /// <summary>查询归一化比例 (0, 1]；档位从 1 开始。</summary>
    public static double Ratio(BrushKind kind, uint level)
    {
        Status status = Native.BrushPreset((uint)kind, level, out double ratio);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return ratio;
    }

    /// <summary>档位换算成实际线宽：maxWidth × ratio。</summary>
    public static double Width(BrushKind kind, uint level, double maxWidth)
    {
        Status status = Native.BrushPresetWidth((uint)kind, level, maxWidth, out double width);
        if (status != Status.Ok) throw new ScratchmarkException(status);
        return width;
    }
}
/* M14 点间距重采样：间距 = 笔宽 × 比例；输出沿用 BrushPoint。 */
[StructLayout(LayoutKind.Sequential)]
public struct ResampleOptions
{
    public double SpacingRatio;
    public BrushOptions Brush;
    public uint Reserved;
}
/* M15 带刷新率的预测选项：RefreshRateHz = 0 表示未提供，行为与 PredictionOptions 一致。 */
[StructLayout(LayoutKind.Sequential)]
public struct PredictionOptionsEx
{
    public double HorizonUs, MaxDistance;
    public uint Count, Reserved;
    public BrushOptions Brush;
    public double RefreshRateHz;
}
/* M17：样本 flags 的位语义。 */
public static class SampleFlags
{
    /// <summary>该点带真实压力（pressure ∈ [0,1]）。</summary>
    public const uint Pressure = 1;

    /// <summary>该点是擦除切出来的端点：渲染时应收成圆缺口而不是平口。</summary>
    public const uint CutEnd = 2;
}
