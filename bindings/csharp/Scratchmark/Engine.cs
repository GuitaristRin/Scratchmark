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

    public void ErasePath(ulong revision, ReadOnlySpan<Point> path, double radius)
    {
        lock (gate)
        fixed (Point* points = path)
            Check(Native.ErasePath(Handle, revision, points, (uint)path.Length, radius));
    }

    /// <summary>套索命中：返回升序笔划索引；命中为零时返回空数组，不产生异常。</summary>
    public uint[] LassoHits(ulong revision, ReadOnlySpan<Point> path)
    {
        lock (gate)
        fixed (Point* points = path)
        {
            uint length = (uint)path.Length;
            // 先按容量 0 查询所需数量：BUFFER_TOO_SMALL 是查询成功的信号，不是错误。
            Status probe = Native.LassoHits(Handle, revision, points, length, null, 0, out uint required);
            if (probe != Status.Ok && probe != Status.BufferTooSmall) Check(probe);
            if (required == 0) return [];
            uint[] buffer = new uint[required];
            fixed (uint* indices = buffer)
                Check(Native.LassoHits(Handle, revision, points, length, indices, required, out _));
            return buffer;
        }
    }

    public void EraseIndices(ulong revision, ReadOnlySpan<uint> indices)
    {
        lock (gate)
        fixed (uint* list = indices)
            Check(Native.EraseIndices(Handle, revision, list, (uint)indices.Length));
    }

    public void Clear(ulong revision)
    {
        lock (gate) Check(Native.Clear(Handle, revision));
    }

    /// <summary>原位变换选区：索引与 z 序不变，线宽按 √|det| 缩放。</summary>
    public void TransformStrokes(ulong revision, ReadOnlySpan<uint> indices, Transform transform)
    {
        lock (gate)
        fixed (uint* list = indices)
            Check(Native.TransformStrokes(Handle, revision, list, (uint)indices.Length, transform));
    }

    /// <summary>复制选区为格式 1 快照字节，可直接当剪贴板内容。</summary>
    public byte[] CopyStrokes(ulong revision, ReadOnlySpan<uint> indices)
    {
        lock (gate)
        fixed (uint* list = indices)
        {
            uint count = (uint)indices.Length;
            Status probe = Native.CopyStrokes(Handle, revision, list, count, null, 0, out uint required);
            if (probe != Status.Ok && probe != Status.BufferTooSmall) Check(probe);
            byte[] buffer = new byte[required];
            fixed (byte* data = buffer)
                Check(Native.CopyStrokes(Handle, revision, list, count, data, required, out _));
            return buffer;
        }
    }

    /// <summary>粘贴格式 1 快照字节；`null` 表示恒等变换。</summary>
    public void Paste(ulong revision, ReadOnlySpan<byte> data, Transform? transform = null)
    {
        lock (gate)
        fixed (byte* bytes = data)
        {
            uint length = (uint)data.Length;
            if (transform is { } value) Check(Native.Paste(Handle, revision, bytes, length, value));
            else Check(Native.PasteIdentity(Handle, revision, bytes, length, IntPtr.Zero));
        }
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

    /// <summary>与坐标同批写入逐点轴向（倾斜 / 朝向）；两数组长度必须一致。</summary>
    public void AppendAxes(ReadOnlySpan<Sample> samples, ReadOnlySpan<SampleAxes> axes)
    {
        if (samples.Length != axes.Length) throw new ArgumentException("samples and axes must have the same length", nameof(axes));
        lock (gate)
        {
            fixed (Sample* points = samples)
            fixed (SampleAxes* values = axes)
                Check(Native.AppendAxes(Handle, points, values, (uint)samples.Length));
        }
    }

    /// <summary>读取逐点轴向；分段与容量语义同 ReadPoints。</summary>
    public uint ReadAxes(ulong revision, uint index, uint start, Span<SampleAxes> destination)
    {
        lock (gate)
        fixed (SampleAxes* values = destination)
        {
            uint written = 0;
            Check(Native.ReadAxes(Handle, revision, index, start, values, (uint)destination.Length, &written));
            return written;
        }
    }

    /// <summary>按笔宽的点间距重采样：只读派生，返回从首点到末点的等距点列。</summary>
    public BrushPoint[] Resample(ulong revision, uint index, ResampleOptions options)
    {
        lock (gate)
        {
            Status probe = Native.ResampleStroke(Handle, revision, index, options, null, 0, out uint required);
            if (probe != Status.Ok && probe != Status.BufferTooSmall) Check(probe);
            if (required == 0) return [];
            BrushPoint[] buffer = new BrushPoint[required];
            fixed (BrushPoint* points = buffer)
                Check(Native.ResampleStroke(Handle, revision, index, options, points, required, out _));
            return buffer;
        }
    }

    /// <summary>预测笔迹：只读外推活动笔划的尾迹，不写文档、不改 revision。</summary>
    public BrushPoint[] Predict(ulong revision, PredictionOptions options)
    {
        lock (gate)
        {
            Status probe = Native.Predict(Handle, revision, options, null, 0, out uint required);
            if (probe != Status.Ok && probe != Status.BufferTooSmall) Check(probe);
            if (required == 0) return [];
            BrushPoint[] buffer = new BrushPoint[required];
            fixed (BrushPoint* points = buffer)
                Check(Native.Predict(Handle, revision, options, points, required, out _));
            return buffer;
        }
    }

    /// <summary>文档边界：已提交笔划的并集（含笔宽）与笔划数；空文档 found 为 false。</summary>
    public (Bounds Bounds, uint Strokes, bool Found) DocumentBounds(ulong revision)
    {
        lock (gate)
        {
            Check(Native.DocumentBounds(Handle, revision, out Bounds bounds, out uint strokes, out uint found));
            return (bounds, strokes, found != 0);
        }
    }

    /// <summary>整份平移：一次调用一个撤销条目，越界整次失败。</summary>
    public void TranslateAll(ulong revision, double dx, double dy)
    {
        lock (gate) Check(Native.TranslateAll(Handle, revision, dx, dy));
    }

    /// <summary>带刷新率的预测：刷新率为 0 时与 Predict 逐点一致。</summary>
    public BrushPoint[] PredictEx(ulong revision, PredictionOptionsEx options)
    {
        lock (gate)
        {
            Status probe = Native.PredictEx(Handle, revision, options, null, 0, out uint required);
            if (probe != Status.Ok && probe != Status.BufferTooSmall) Check(probe);
            if (required == 0) return [];
            BrushPoint[] buffer = new BrushPoint[required];
            fixed (BrushPoint* points = buffer)
                Check(Native.PredictEx(Handle, revision, options, points, required, out _));
            return buffer;
        }
    }

    /// <summary>形状规整：preview 为真时只测量残差，不写文档、不占历史。</summary>
    public ShapeInfo RegularizeStroke(ulong revision, uint index, ShapeKind kind, bool preview = false)
    {
        lock (gate)
        {
            Check(Native.RegularizeStroke(Handle, revision, index, (uint)kind, preview ? 1u : 0u, out ShapeInfo info));
            return info;
        }
    }
}
