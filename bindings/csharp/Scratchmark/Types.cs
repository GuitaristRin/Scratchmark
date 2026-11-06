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

public sealed class ScratchmarkException(Status status) : Exception($"Scratchmark: {status}")
{
    public Status Status { get; } = status;
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