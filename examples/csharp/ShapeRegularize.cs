using Scratchmark;

internal static partial class Program
{
    private static void Shape(string dll)
    {
        using var engine = new Engine(dll);
        CommitLine(engine, 0);
        State state = engine.GetState();

        // 预览：只测量残差，不改文档、不占历史。
        ShapeInfo preview = engine.RegularizeStroke(state.Revision, 0, ShapeKind.Line, preview: true);
        Require(preview.Kind == (uint)ShapeKind.Line && preview.PointCount == 2, "line preview reports two points");
        Require(preview.Error < 1e-9, "straight stroke fits a line exactly");
        Require(engine.GetState().Revision == state.Revision, "preview does not change the document");

        // 实际替换：一个撤销条目，颜色与线宽继承。
        state = engine.GetState();
        engine.RegularizeStroke(state.Revision, 0, ShapeKind.Line);
        state = engine.GetState();
        StrokeInfo line = engine.GetStroke(state.Revision, 0);
        Require(line.PointCount == 2 && line.Width == 2, "regularized line keeps width and collapses to two points");
        engine.Undo();
        Require(engine.GetStroke(engine.GetState().Revision, 0).PointCount == 2, "undo restores the hand-drawn stroke");

        // 箭头：3 笔 6 点，占用原索引并把后面的笔划顺延。
        CommitArrowSource(engine);
        state = engine.GetState();
        ShapeInfo arrow = engine.RegularizeStroke(state.Revision, 1, ShapeKind.Arrow);
        Require(arrow.StrokeCount == 3 && arrow.PointCount == 6, "arrow reports three strokes and six points");
        state = engine.GetState();
        Require(state.StrokeCount == 4, "arrow adds two head strokes on top of the shaft");
        Require(engine.GetStroke(state.Revision, 1).PointCount == 2, "shaft stays at the original index");

        // 矩形与圆：轴对齐包围盒 / Kåsa 圆，圆固定 65 点。
        CommitRectangleSource(engine);
        state = engine.GetState();
        ShapeInfo rectangle = engine.RegularizeStroke(state.Revision, 3, ShapeKind.Rectangle);
        Require(rectangle.PointCount == 5, "rectangle is a closed loop of five points");
        state = engine.GetState();
        Require(engine.GetStroke(state.Revision, 3).PointCount == 5, "rectangle replaces the stroke in place");

        engine.Begin(2, 0);
        engine.Append([new() { X = 300, Y = 300, TimeUs = 0 }, new() { X = 340, Y = 300, TimeUs = 1 },
            new() { X = 340, Y = 340, TimeUs = 2 }, new() { X = 300, Y = 340, TimeUs = 3 }]);
        engine.Finish();
        state = engine.GetState();
        uint last = state.StrokeCount - 1;
        ShapeInfo circle = engine.RegularizeStroke(state.Revision, last, ShapeKind.Circle);
        Require(circle.PointCount == 65, "circle is sampled into 65 closed points");
        state = engine.GetState();
        Require(engine.GetStroke(state.Revision, last).PointCount == 65, "circle replaces the stroke in place");

        // 退化输入：单点笔划不能规整成直线，且失败不改变文档。
        engine.Begin(2, 0);
        engine.Append([new() { X = 0, Y = 500, TimeUs = 0 }]);
        engine.Finish();
        state = engine.GetState();
        Expect(Status.InvalidArgument, () => engine.RegularizeStroke(state.Revision, state.StrokeCount - 1, ShapeKind.Line));
        Require(engine.GetState().Revision == state.Revision, "failed regularization preserves revision");
        Expect(Status.StaleRevision, () => engine.RegularizeStroke(state.Revision - 1, 0, ShapeKind.Line, preview: true));
    }

    private static void CommitArrowSource(Engine engine)
    {
        engine.Begin(2, 0);
        engine.Append([new() { X = 0, Y = 300, TimeUs = 0 }, new() { X = 50, Y = 300, TimeUs = 1_000 },
            new() { X = 100, Y = 300, TimeUs = 2_000 }]);
        engine.Finish();
    }

    private static void CommitRectangleSource(Engine engine)
    {
        engine.Begin(2, 0);
        engine.Append([new() { X = 0, Y = 0, TimeUs = 0 }, new() { X = 10, Y = 0, TimeUs = 1 },
            new() { X = 10, Y = 10, TimeUs = 2 }, new() { X = 0, Y = 10, TimeUs = 3 }]);
        engine.Finish();
    }
}
