using Scratchmark;

internal static partial class Program
{
    private static PredictionOptions Prediction(double horizonUs = 16_000, uint count = 4, double maxDistance = 60) => new()
    {
        HorizonUs = horizonUs,
        MaxDistance = maxDistance,
        Count = count,
        Brush = new BrushOptions { Alpha = 1.0, MinWidthRatio = 1.0, PressureEnabled = 0 },
    };

    private static void PredictionChecks(string dll)
    {
        using var engine = new Engine(dll);

        // 没有活动笔划：成功但零预测点。
        Require(engine.Predict(engine.GetState().Revision, Prediction()).Length == 0, "no active stroke yields no prediction");

        // 匀速活动笔划：0.1 页面单位 / 微秒，时间窗 1000 微秒。
        engine.Begin(2, 0x11223344);
        engine.Append([new() { X = 0, Y = 0, TimeUs = 0 }, new() { X = 50, Y = 0, TimeUs = 500 },
            new() { X = 100, Y = 0, TimeUs = 1_000 }]);
        State active = engine.GetState();
        // 上限给足 1000，让速度（100 单位）不被收缩，便于断言手算值。
        BrushPoint[] predicted = engine.Predict(active.Revision, Prediction(1_000, 4, 1_000));
        Require(predicted.Length == 4, "prediction returns the requested point count");
        Require(Math.Abs(predicted[0].X - 125) < 1e-9 && Math.Abs(predicted[3].X - 200) < 1e-9,
            "straight prediction follows the velocity");
        Require(Math.Abs(predicted[0].Width - 2) < 1e-9, "prediction keeps the stroke width");
        Require(engine.GetState().Revision == active.Revision && engine.GetState().ActivePoints == 3,
            "prediction does not change the document");

        // 长度上限：总位移收缩到 max_distance。
        BrushPoint[] capped = engine.Predict(active.Revision, Prediction(1_000, 5, 10));
        Require(capped.Length == 5 && Math.Abs(capped[4].X - 110) < 1e-9, "max distance caps the prediction");

        // 非法选项被拒绝。
        Expect(Status.InvalidArgument, () => engine.Predict(active.Revision, Prediction(0, 4)));
        Expect(Status.InvalidArgument, () => engine.Predict(active.Revision, Prediction(1_000, 0)));
        Expect(Status.StaleRevision, () => engine.Predict(active.Revision - 1, Prediction(1_000, 4)));

        // 提交后预测为空，且不影响编辑历史。
        engine.Finish();
        State committed = engine.GetState();
        Require(engine.Predict(committed.Revision, Prediction()).Length == 0, "committed stroke has no prediction");
        Require(engine.GetState().CanUndo == 1, "prediction does not touch history");
        engine.Undo();
        Require(engine.GetState().StrokeCount == 0, "undo still works after predictions");
    }
}
