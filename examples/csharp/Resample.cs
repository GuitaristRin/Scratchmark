using Scratchmark;

internal static partial class Program
{
    private static ResampleOptions Resample(double ratio, bool pressure = false) => new()
    {
        SpacingRatio = ratio,
        Brush = new BrushOptions { Alpha = 1.0, MinWidthRatio = pressure ? 0.2 : 1.0, PressureEnabled = pressure ? 1u : 0u },
    };

    private static void ResampleCheck(string dll)
    {
        using var engine = new Engine(dll);
        CommitLine(engine, 0);
        State state = engine.GetState();

        // 线宽 2、比例 0.5 → 间距 1 → 0..10 共 11 点。
        BrushPoint[] points = engine.Resample(state.Revision, 0, Resample(0.5));
        Require(points.Length == 11, "resample count follows spacing");
        Require(points[0].X == 0 && Math.Abs(points[5].X - 5) < 1e-9 && Math.Abs(points[10].X - 10) < 1e-9,
            "resample positions are evenly spaced and include the end point");
        Require(Math.Abs(points[10].Width - 2) < 1e-9, "resample keeps the stroke width");

        // 比例 1.25 → 间距 2.5 → 0/2.5/5/7.5/10 共 5 点。
        Require(engine.Resample(state.Revision, 0, Resample(1.25)).Length == 5, "wider spacing yields fewer points");

        // 只读：文档不变。
        Require(engine.GetState().Revision == state.Revision, "resample does not change the document");

        // 压力笔划：宽度沿笔划插值。
        engine.Begin(10, 0x9C2B2BFF);
        engine.Append([new() { X = 0, Y = 50, TimeUs = 0, Pressure = 0f, Flags = 1 },
            new() { X = 10, Y = 50, TimeUs = 100, Pressure = 1f, Flags = 1 }]);
        engine.Finish();
        state = engine.GetState();
        uint last = state.StrokeCount - 1;
        BrushPoint[] pressure = engine.Resample(state.Revision, last, Resample(0.3, pressure: true));
        Require(pressure.Length == 5, "pressure resample count");
        Require(Math.Abs(pressure[0].Width - 2) < 1e-6 && Math.Abs(pressure[4].Width - 10) < 1e-6,
            "resample interpolates brush width");

        // 非法参数被拒绝。
        Expect(Status.InvalidArgument, () => engine.Resample(state.Revision, last, Resample(0)));
        Expect(Status.InvalidArgument, () => engine.Resample(state.Revision, 99, Resample(0.5)));
        Expect(Status.StaleRevision, () => engine.Resample(state.Revision - 1, 0, Resample(0.5)));
    }
}
