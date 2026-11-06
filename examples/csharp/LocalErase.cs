using Scratchmark;

internal static partial class Program
{
    private static void CommitHorizontal(Engine engine, double x, double y)
    {
        engine.Begin(2, 0x11223344);
        engine.Append([new() { X = x, Y = y, TimeUs = 0 }, new() { X = x + 10, Y = y, TimeUs = 1000 }]);
        engine.Finish();
    }

    private static void LocalErase(string dll)
    {
        using var engine = new Engine(dll);
        CommitHorizontal(engine, 0, 0);
        CommitHorizontal(engine, 0, 100);
        State state = engine.GetState();

        // 半径 2 的单点擦除把第一笔切成 [0,3] 与 [7,10]，第二笔不受影响。
        engine.ErasePath(state.Revision, [new() { X = 5, Y = 0 }], 2);
        state = engine.GetState();
        Require(state.StrokeCount == 3, "partial erase splits one stroke");
        Require(state.CanUndo == 1 && state.CanRedo == 0, "partial erase is undoable");

        StrokeInfo first = engine.GetStroke(state.Revision, 0);
        StrokeInfo second = engine.GetStroke(state.Revision, 1);
        Require(first.MaxX == 4 && first.PointCount == 2, "first fragment ends at the cut");
        Require(second.MinX == 6 && second.PointCount == 2, "second fragment starts at the cut");
        Require(!engine.HitTest(state.Revision, 5, 0, 0).Found, "the erased gap is not hittable");
        Require(engine.HitTest(state.Revision, 8, 0, 0).Found, "the surviving fragment is hittable");

        // 失败路径不改变文档。
        Expect(Status.StaleRevision, () => engine.ErasePath(state.Revision - 1, [new() { X = 5, Y = 0 }], 2));
        Expect(Status.InvalidArgument, () => engine.ErasePath(state.Revision, [], 2));
        Expect(Status.InvalidArgument, () => engine.ErasePath(state.Revision, [new() { X = 5, Y = 0 }], 0));
        Require(engine.GetState().Revision == state.Revision, "failed erase preserves revision");

        // 活动笔划期间拒绝，取消后恢复可用。
        engine.Begin(2, 0);
        engine.Append([new() { X = 0, Y = -10, TimeUs = 0 }]);
        Expect(Status.InvalidState, () => engine.ErasePath(engine.GetState().Revision, [new() { X = 5, Y = 0 }], 2));
        engine.Cancel();

        // 一次手势只占一个撤销条目：撤销恢复原位，重做再次切分。
        engine.Undo();
        state = engine.GetState();
        Require(state.StrokeCount == 2, "undo restores both original strokes");
        Require(engine.GetStroke(state.Revision, 0).MaxX == 11, "undo restores the original stroke");
        engine.Redo();
        state = engine.GetState();
        Require(state.StrokeCount == 3, "redo splits again");

        // 相切不切分，也不递增 revision。
        engine.Undo();
        state = engine.GetState();
        engine.ErasePath(state.Revision, [new() { X = 5, Y = 2 }], 2);
        Require(engine.GetState().Revision == state.Revision, "tangent touch does not split");

        // 整笔被覆盖时整笔消失。
        engine.ErasePath(engine.GetState().Revision, [new() { X = 0, Y = 100 }], 20);
        state = engine.GetState();
        Require(state.StrokeCount == 1, "fully covered stroke disappears");

        // 局部擦除后的片段可整笔删除并撤销回来。
        state = engine.GetState();
        engine.EraseStroke(state.Revision, 0);
        Require(engine.GetState().StrokeCount == 0, "fragment can be erased as a whole stroke");
        engine.Undo();
        Require(engine.GetState().StrokeCount == 1, "whole-stroke erase of a fragment is undoable");
    }
}
