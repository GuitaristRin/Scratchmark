using Scratchmark;

internal static partial class Program
{
    private static void Selection(string dll)
    {
        using var engine = new Engine(dll);
        CommitLine(engine, 0);
        CommitLine(engine, 100);
        CommitLine(engine, 500);
        State state = engine.GetState();

        // 套索只做只读命中查询：框住前两笔。
        Point[] lasso = [new() { X = -20, Y = -20 }, new() { X = 200, Y = -20 },
            new() { X = 200, Y = 20 }, new() { X = -20, Y = 20 }];
        uint[] hits = engine.LassoHits(state.Revision, lasso);
        Require(hits.Length == 2 && hits[0] == 0 && hits[1] == 1, "lasso selects two strokes in z order");
        Require(engine.GetState().Revision == state.Revision, "lasso query does not change the document");

        // 一次调用只占一个撤销条目。
        engine.EraseIndices(state.Revision, hits);
        Require(engine.GetState().StrokeCount == 1, "erase indices removes the selection at once");
        Require(engine.GetState().CanUndo == 1 && engine.GetState().CanRedo == 0, "selection delete is undoable");
        engine.Undo();
        Require(engine.GetState().StrokeCount == 3, "one undo restores the whole selection");
        engine.Redo();
        Require(engine.GetState().StrokeCount == 1, "redo deletes the selection again");

        // 失败路径：重复索引、越界、过期 revision。
        state = engine.GetState();
        Expect(Status.InvalidArgument, () => engine.EraseIndices(state.Revision, [0, 0]));
        Expect(Status.InvalidArgument, () => engine.EraseIndices(state.Revision, [0, 7]));
        Expect(Status.StaleRevision, () => engine.EraseIndices(state.Revision - 1, [0]));
        Expect(Status.InvalidArgument, () => engine.LassoHits(state.Revision, [new() { X = 0, Y = 0 }, new() { X = 1, Y = 1 }]));
        Require(engine.GetState().Revision == state.Revision, "failed selection calls preserve revision");

        // 活动笔划期间拒绝删除。
        engine.Begin(2, 0);
        engine.Append([new() { X = 0, Y = 30, TimeUs = 0 }]);
        Expect(Status.InvalidState, () => engine.EraseIndices(engine.GetState().Revision, [0]));
        engine.Cancel();

        // 清空是一个条目，一次撤销全回来。
        state = engine.GetState();
        engine.Clear(state.Revision);
        Require(engine.GetState().StrokeCount == 0, "clear removes everything");
        engine.Undo();
        Require(engine.GetState().StrokeCount == 1, "one undo restores the cleared document");
        state = engine.GetState();
        engine.Clear(state.Revision);
        state = engine.GetState();
        engine.Clear(state.Revision);
        Require(engine.GetState().Revision == state.Revision, "clearing an empty document keeps revision");
    }
}
