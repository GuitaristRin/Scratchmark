using Scratchmark;

internal static partial class Program
{
    private static PredictionOptionsEx PredictEx(double horizonUs, double refreshRateHz, uint count = 4) => new()
    {
        HorizonUs = horizonUs,
        MaxDistance = 10_000,
        Count = count,
        Brush = new BrushOptions { Alpha = 1.0, MinWidthRatio = 1.0, PressureEnabled = 0 },
        RefreshRateHz = refreshRateHz,
    };

    private static void PredictionRefresh(string dll)
    {
        using var engine = new Engine(dll);
        engine.Begin(2, 0x11223344);
        engine.Append([new() { X = 0, Y = 0, TimeUs = 0 }, new() { X = 50, Y = 0, TimeUs = 500 },
            new() { X = 100, Y = 0, TimeUs = 1_000 }]);
        State active = engine.GetState();

        // 刷新率为 0：与 M10 的 Predict 逐点一致。
        BrushPoint[] legacy = engine.Predict(active.Revision, new PredictionOptions
        {
            HorizonUs = 1_000,
            MaxDistance = 10_000,
            Count = 4,
            Brush = new BrushOptions { Alpha = 1.0, MinWidthRatio = 1.0, PressureEnabled = 0 },
        });
        BrushPoint[] zero = engine.PredictEx(active.Revision, PredictEx(1_000, 0));
        Require(legacy.Length == zero.Length && zero.Length == 4, "zero refresh matches M10 count");
        for (int i = 0; i < legacy.Length; i++)
            Require(legacy[i].X == zero[i].X && legacy[i].Width == zero[i].Width, "zero refresh matches M10 points");

        // 240 Hz 把时间窗压到一帧（≈4166.67µs），预测明显变短。
        BrushPoint[] longWindow = engine.PredictEx(active.Revision, PredictEx(16_000, 0));
        BrushPoint[] oneFrame = engine.PredictEx(active.Revision, PredictEx(16_000, 240));
        Require(oneFrame[3].X < longWindow[3].X, "refresh rate shortens the prediction");
        Require(Math.Abs(longWindow[3].X - 1700) < 1e-6, "16ms window extrapolates 1600 units");
        Require(Math.Abs(oneFrame[3].X - 516.6666667) < 1e-6, "one frame at 240Hz extrapolates 416.67 units");

        // 非法刷新率被拒绝。
        Expect(Status.InvalidArgument, () => engine.PredictEx(active.Revision, PredictEx(16_000, -1)));
        Expect(Status.InvalidArgument, () => engine.PredictEx(active.Revision, PredictEx(16_000, double.NaN)));
        Expect(Status.InvalidArgument, () => engine.PredictEx(active.Revision, PredictEx(16_000, 10_001)));
        Expect(Status.StaleRevision, () => engine.PredictEx(active.Revision - 1, PredictEx(16_000, 240)));

        // 预测仍不改变文档。
        Require(engine.GetState().Revision == active.Revision && engine.GetState().ActivePoints == 3,
            "refresh prediction does not change the document");
        engine.Cancel();
    }
}
