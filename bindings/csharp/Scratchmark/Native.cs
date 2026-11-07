using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

[assembly: InternalsVisibleTo("Scratchmark.Smoke")]

namespace Scratchmark;

internal static unsafe class Native
{
    private const string Library = "scratchmark";
    private static readonly object Gate = new();
    private static IntPtr library;
    private static string? loadedPath;

    static Native() => NativeLibrary.SetDllImportResolver(typeof(Native).Assembly, Resolve);

    private static IntPtr Resolve(string name, Assembly assembly, DllImportSearchPath? searchPath)
        => name == Library ? library : IntPtr.Zero;

    internal static void Load(string path)
    {
        if (!OperatingSystem.IsWindows() || RuntimeInformation.ProcessArchitecture != Architecture.X64)
            throw new PlatformNotSupportedException("M1 requires Windows x64.");
        if (!Path.IsPathFullyQualified(path)) throw new ArgumentException("Absolute DLL path required.", nameof(path));
        path = Path.GetFullPath(path);
        lock (Gate)
        {
            if (library != IntPtr.Zero && !string.Equals(path, loadedPath, StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException("A different SDK is already loaded in this process.");
            if (library == IntPtr.Zero)
            {
                library = NativeLibrary.Load(path);
                loadedPath = path;
            }
            // 库保持到进程结束，避免 finalizer 或在途调用访问已卸载代码。
            if (AbiVersion() != 1) throw new NotSupportedException("Expected Scratchmark ABI 1.");
            if (!NativeLibrary.TryGetExport(library, "sm_features", out _) || (Features() & 2047) != 2047)
                throw new NotSupportedException("SDK does not support M2 brush, M3 page/viewport, M4 hit/erase, M5 partial erase, M6 selection, M7 transform, M8 shape, M9 viewport rotation, M10 prediction, M11 stylus axes or M12 page extent.");
        }
    }

    [DllImport(Library, EntryPoint = "sm_abi_version", CallingConvention = CallingConvention.Cdecl)]
    internal static extern uint AbiVersion();
    [DllImport(Library, EntryPoint = "sm_features", CallingConvention = CallingConvention.Cdecl)]
    internal static extern ulong Features();
    [DllImport(Library, EntryPoint = "sm_create", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Create(out ulong handle);
    [DllImport(Library, EntryPoint = "sm_destroy", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Destroy(ulong handle);
    [DllImport(Library, EntryPoint = "sm_begin", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Begin(ulong handle, double width, uint color);
    [DllImport(Library, EntryPoint = "sm_append", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Append(ulong handle, Sample* samples, uint count);
    [DllImport(Library, EntryPoint = "sm_finish", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Finish(ulong handle);
    [DllImport(Library, EntryPoint = "sm_cancel", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Cancel(ulong handle);
    [DllImport(Library, EntryPoint = "sm_undo", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Undo(ulong handle);
    [DllImport(Library, EntryPoint = "sm_redo", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Redo(ulong handle);
    [DllImport(Library, EntryPoint = "sm_get_state", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status GetState(ulong handle, out State state);
    [DllImport(Library, EntryPoint = "sm_get_stroke", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status GetStroke(ulong handle, ulong revision, uint index, out StrokeInfo info);
    [DllImport(Library, EntryPoint = "sm_read_points", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ReadPoints(ulong handle, ulong revision, uint index, uint start,
        Sample* points, uint capacity, uint* written);
    [DllImport(Library, EntryPoint = "sm_save", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Save(ulong handle, byte* bytes, uint capacity, out uint required);
    [DllImport(Library, EntryPoint = "sm_load", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status LoadSnapshot(ulong handle, byte* bytes, uint length);
    [DllImport(Library, EntryPoint = "sm_read_brush_points", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ReadBrushPoints(ulong handle, ulong revision, uint index, uint start,
        BrushOptions* options, BrushPoint* points, uint capacity, uint* written);
    [DllImport(Library, EntryPoint = "sm_get_page_id", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status GetPageId(ulong handle, out ulong pageId);
    [DllImport(Library, EntryPoint = "sm_set_page_id", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status SetPageId(ulong handle, ulong pageId);
    [DllImport(Library, EntryPoint = "sm_hit_test", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status HitTest(ulong handle, ulong revision, double x, double y, double tolerance,
        out uint index, out uint found);
    [DllImport(Library, EntryPoint = "sm_erase_stroke", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status EraseStroke(ulong handle, ulong revision, uint index);
    [DllImport(Library, EntryPoint = "sm_erase_path", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ErasePath(ulong handle, ulong revision, Point* path, uint count, double radius);
    [DllImport(Library, EntryPoint = "sm_lasso_hits", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status LassoHits(ulong handle, ulong revision, Point* path, uint count,
        uint* indices, uint capacity, out uint written);
    [DllImport(Library, EntryPoint = "sm_erase_indices", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status EraseIndices(ulong handle, ulong revision, uint* indices, uint count);
    [DllImport(Library, EntryPoint = "sm_clear", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Clear(ulong handle, ulong revision);
    [DllImport(Library, EntryPoint = "sm_transform_strokes", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status TransformStrokes(ulong handle, ulong revision, uint* indices, uint count, in Transform transform);
    [DllImport(Library, EntryPoint = "sm_copy_strokes", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status CopyStrokes(ulong handle, ulong revision, uint* indices, uint count,
        byte* data, uint capacity, out uint required);
    [DllImport(Library, EntryPoint = "sm_paste", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Paste(ulong handle, ulong revision, byte* data, uint length, in Transform transform);
    [DllImport(Library, EntryPoint = "sm_paste", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status PasteIdentity(ulong handle, ulong revision, byte* data, uint length, IntPtr transform);
    [DllImport(Library, EntryPoint = "sm_regularize_stroke", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status RegularizeStroke(ulong handle, ulong revision, uint index, uint kind, uint flags, out ShapeInfo info);
    [DllImport(Library, EntryPoint = "sm_page_to_viewport", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status PageToViewport(in Viewport viewport, in Point input, out Point output);
    [DllImport(Library, EntryPoint = "sm_viewport_to_page", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ViewportToPage(in Viewport viewport, in Point input, out Point output);
    [DllImport(Library, EntryPoint = "sm_width_page_to_viewport", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status WidthPageToViewport(in Viewport viewport, double width, out double output);
    [DllImport(Library, EntryPoint = "sm_width_viewport_to_page", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status WidthViewportToPage(in Viewport viewport, double width, out double output);
    [DllImport(Library, EntryPoint = "sm_bounds_page_to_viewport", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status BoundsPageToViewport(in Viewport viewport, in Bounds input, out Bounds output);
    [DllImport(Library, EntryPoint = "sm_page_to_viewport_ex", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status PageToViewportEx(in ViewportEx viewport, in Point input, out Point output);
    [DllImport(Library, EntryPoint = "sm_viewport_to_page_ex", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ViewportToPageEx(in ViewportEx viewport, in Point input, out Point output);
    [DllImport(Library, EntryPoint = "sm_width_page_to_viewport_ex", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status WidthPageToViewportEx(in ViewportEx viewport, double width, out double output);
    [DllImport(Library, EntryPoint = "sm_width_viewport_to_page_ex", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status WidthViewportToPageEx(in ViewportEx viewport, double width, out double output);
    [DllImport(Library, EntryPoint = "sm_bounds_page_to_viewport_ex", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status BoundsPageToViewportEx(in ViewportEx viewport, in Bounds input, out Bounds output);
    [DllImport(Library, EntryPoint = "sm_viewport_ex_zoom_at", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ViewportExZoomAt(in ViewportEx viewport, in Point anchor, double factor, out ViewportEx output);
    [DllImport(Library, EntryPoint = "sm_viewport_ex_fit", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ViewportExFit(in Bounds page, double viewWidth, double viewHeight, double angle, double padding, out ViewportEx output);
    [DllImport(Library, EntryPoint = "sm_predict", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status Predict(ulong handle, ulong revision, in PredictionOptions options,
        BrushPoint* points, uint capacity, out uint written);
    [DllImport(Library, EntryPoint = "sm_append_axes", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status AppendAxes(ulong handle, Sample* samples, SampleAxes* axes, uint count);
    [DllImport(Library, EntryPoint = "sm_read_axes", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ReadAxes(ulong handle, ulong revision, uint index, uint start,
        SampleAxes* axes, uint capacity, uint* written);
    [DllImport(Library, EntryPoint = "sm_document_bounds", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status DocumentBounds(ulong handle, ulong revision, out Bounds bounds,
        out uint strokes, out uint found);
    [DllImport(Library, EntryPoint = "sm_translate_all", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status TranslateAll(ulong handle, ulong revision, double dx, double dy);
    [DllImport(Library, EntryPoint = "sm_brush_preset", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status BrushPreset(uint kind, uint level, out double ratio);
    [DllImport(Library, EntryPoint = "sm_brush_preset_width", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status BrushPresetWidth(uint kind, uint level, double maxWidth, out double width);
    [DllImport(Library, EntryPoint = "sm_resample_stroke", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status ResampleStroke(ulong handle, ulong revision, uint index,
        in ResampleOptions options, BrushPoint* points, uint capacity, out uint written);
    [DllImport(Library, EntryPoint = "sm_predict_ex", CallingConvention = CallingConvention.Cdecl)]
    internal static extern Status PredictEx(ulong handle, ulong revision, in PredictionOptionsEx options,
        BrushPoint* points, uint capacity, out uint written);
}
