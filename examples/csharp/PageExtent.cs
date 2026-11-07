using Scratchmark;

internal static partial class Program
{
    private static void PageExtent(string dll)
    {
        using var engine = new Engine(dll);

        // 空文档没有边界。
        State state = engine.GetState();
        var empty = engine.DocumentBounds(state.Revision);
        Require(!empty.Found && empty.Strokes == 0, "empty document has no bounds");
        Require(empty.Bounds.MinX == 0 && empty.Bounds.MaxY == 0, "empty bounds are zeroed");

        CommitLine(engine, 0);
        CommitLine(engine, 100);
        state = engine.GetState();
        var bounds = engine.DocumentBounds(state.Revision);
        Require(bounds.Found && bounds.Strokes == 2, "bounds count committed strokes");
        // CommitLine 的笔宽是 2，所以左侧多出 1。
        Require(bounds.Bounds.MinX == -1 && bounds.Bounds.MaxX == 111, "bounds include stroke width");

        // 活动笔划不参与边界。
        engine.Begin(2, 0);
        engine.Append([new() { X = 900, Y = 900, TimeUs = 0 }]);
        var active = engine.DocumentBounds(engine.GetState().Revision);
        Require(active.Bounds.MaxX == 111, "active stroke is excluded from bounds");
        engine.Cancel();

        // 整份平移：一次撤销整体回退。
        state = engine.GetState();
        double beforeY = engine.DocumentBounds(state.Revision).Bounds.MinY;
        engine.TranslateAll(state.Revision, 0, 40);
        state = engine.GetState();
        Require(engine.DocumentBounds(state.Revision).Bounds.MinY == beforeY + 40, "translate moves every stroke");
        engine.Undo();
        state = engine.GetState();
        Require(engine.DocumentBounds(state.Revision).Bounds.MinY == beforeY, "translate is one undo entry");

        // 越界与过期 revision。
        Expect(Status.InvalidArgument, () => engine.TranslateAll(state.Revision, 1e9, 0));
        Expect(Status.StaleRevision, () => engine.TranslateAll(state.Revision - 1, 1, 1));
        Expect(Status.StaleRevision, () => { var _ = engine.DocumentBounds(state.Revision - 1); });
        Require(engine.DocumentBounds(state.Revision).Bounds.MinY == beforeY, "failed translate keeps geometry");
    }
}
