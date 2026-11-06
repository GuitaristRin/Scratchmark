namespace Scratchmark;

public sealed unsafe class Engine : IDisposable
{
    private readonly object gate = new();
    private ulong handle;

    public Engine(string dllPath)
    {
        Native.Load(dllPath);
        Check(Native.Create(out handle));
    }

    private ulong Handle
    {
        get { ObjectDisposedException.ThrowIf(handle == 0, this); return handle; }
    }

    private void Check(Status status)
    {
        if (status == Status.Ok) return;
        if (status == Status.Internal) Release();
        throw new ScratchmarkException(status);
    }

    public void Begin(double width, uint color) { lock (gate) Check(Native.Begin(Handle, width, color)); }
    public void Finish() { lock (gate) Check(Native.Finish(Handle)); }
    public void Cancel() { lock (gate) Check(Native.Cancel(Handle)); }
    public void Undo() { lock (gate) Check(Native.Undo(Handle)); }
    public void Redo() { lock (gate) Check(Native.Redo(Handle)); }

    public void Append(ReadOnlySpan<Sample> samples)
    {
        lock (gate)
        fixed (Sample* points = samples)
            Check(Native.Append(Handle, points, (uint)samples.Length));
    }

    public State GetState()
    {
        lock (gate) { Check(Native.GetState(Handle, out State state)); return state; }
    }

    public StrokeInfo GetStroke(ulong revision, uint index)
    {
        lock (gate) { Check(Native.GetStroke(Handle, revision, index, out StrokeInfo info)); return info; }
    }

    public uint ReadPoints(ulong revision, uint index, uint start, Span<Sample> destination)
    {
        lock (gate)
        fixed (Sample* points = destination)
        {
            uint written = 0;
            Check(Native.ReadPoints(Handle, revision, index, start, points, (uint)destination.Length, &written));
            return written;
        }
    }

    public uint ReadBrushPoints(ulong revision, uint index, uint start, BrushOptions options, Span<BrushPoint> destination)
    {
        lock (gate)
        fixed (BrushPoint* points = destination)
        {
            uint written = 0;
            Check(Native.ReadBrushPoints(Handle, revision, index, start, &options, points, (uint)destination.Length, &written));
            return written;
        }
    }

    public ulong GetPageId()
    {
        lock (gate) { Check(Native.GetPageId(Handle, out ulong pageId)); return pageId; }
    }

    public void SetPageId(ulong pageId)
    {
        lock (gate) Check(Native.SetPageId(Handle, pageId));
    }

    public HitResult HitTest(ulong revision, double x, double y, double tolerance)
    {
        lock (gate)
        {
            Check(Native.HitTest(Handle, revision, x, y, tolerance, out uint index, out uint found));
            return new HitResult(found != 0, index);
        }
    }

    public void EraseStroke(ulong revision, uint index)
    {
        lock (gate) Check(Native.EraseStroke(Handle, revision, index));
    }

    public byte[] Save()
    {
        lock (gate)
        {
            Status status = Native.Save(Handle, null, 0, out uint required);
            if (status != Status.BufferTooSmall) Check(status);
            if (required > 64 * 1024 * 1024) throw new ScratchmarkException(Status.LimitExceeded);
            byte[] bytes = new byte[checked((int)required)];
            fixed (byte* buffer = bytes) Check(Native.Save(Handle, buffer, required, out _));
            return bytes;
        }
    }

    public void Load(ReadOnlySpan<byte> bytes)
    {
        lock (gate)
        fixed (byte* buffer = bytes)
            Check(Native.LoadSnapshot(Handle, buffer, (uint)bytes.Length));
    }

    private Status Release()
    {
        ulong previous = handle;
        handle = 0;
        return previous == 0 ? Status.Ok : Native.Destroy(previous);
    }

    public void Dispose()
    {
        try { lock (gate) Check(Release()); }
        finally { GC.SuppressFinalize(this); }
    }

    ~Engine()
    {
        // finalizer 不承诺及时；同时不得把清理异常传播到宿主进程。
        try { lock (gate) Release(); }
        catch { }
    }
}
