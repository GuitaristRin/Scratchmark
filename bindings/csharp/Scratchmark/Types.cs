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
