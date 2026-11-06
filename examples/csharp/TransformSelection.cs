using Scratchmark;

internal static partial class Program
{
    private static void TransformSelection(string dll)
    {
        using var engine = new Engine(dll);
        CommitLine(engine, 0);
        CommitLine(engine, 100);
        State state = engine.GetState();

        // 原位平移：索引与 z 序不变，未被选中的笔划不动。
        engine.TransformStrokes(state.Revision, [0u], Transform.Translate(10, 5));
        state = engine.GetState();
        Require(state.StrokeCount == 2, "transform keeps stroke count");
        StrokeInfo moved = engine.GetStroke(state.Revision, 0);
        Require(moved.MinX == 9 && moved.MinY == 4 && moved.Width == 2, "translate moves the selected stroke");
        Require(engine.GetStroke(state.Revision, 1).MinX == 99, "unselected stroke stays put");
        engine.Undo();
        Require(engine.GetStroke(engine.GetState().Revision, 0).MinX == -1, "transform is one undo entry");
        engine.Redo();

        // 等比缩放 2×：det = 4，线宽 ×2。
        state = engine.GetState();
        engine.Undo();
        state = engine.GetState();
        engine.TransformStrokes(state.Revision, [0u], Transform.Scale(2, 2));
        state = engine.GetState();
        StrokeInfo scaled = engine.GetStroke(state.Revision, 0);
        Require(scaled.MinX == -2 && scaled.Width == 4, "uniform scale doubles width and coordinates");
        engine.Undo();

        // 非法矩阵：退化与非有限值都拒绝，且文档不变。
        state = engine.GetState();
        Expect(Status.InvalidArgument, () => engine.TransformStrokes(state.Revision, [0u], new Transform { M11 = 0 }));
        Expect(Status.InvalidArgument, () => engine.TransformStrokes(state.Revision, [0u], new Transform { M00 = double.NaN, M11 = 1 }));
        Expect(Status.InvalidArgument, () => engine.TransformStrokes(state.Revision, [0u, 0u], Transform.Translate(1, 0)));
        Require(engine.GetState().Revision == state.Revision, "failed transform preserves revision");

        // 复制 → 粘贴：剪贴板就是格式 1 快照字节。
        byte[] clipboard = engine.CopyStrokes(state.Revision, [0u, 1u]);
        Require(clipboard.Length > 8 && clipboard[0] == (byte)'S' && clipboard[7] == 0, "clipboard is a format-1 snapshot");
        engine.Paste(state.Revision, clipboard, Transform.Translate(0, 200));
        state = engine.GetState();
        Require(state.StrokeCount == 4, "paste appends the batch on top");
        Require(engine.GetStroke(state.Revision, 2).MinY == 199, "paste applies the transform");
        engine.Undo();
        Require(engine.GetState().StrokeCount == 2, "paste is one undo entry");
        engine.Redo();
        Require(engine.GetState().StrokeCount == 4, "paste redo restores the batch");

        // 格式 2（整份文档）不能当剪贴板内容。
        engine.SetPageId(7);
        byte[] document = engine.Save();
        state = engine.GetState();
        Expect(Status.InvalidData, () => engine.Paste(state.Revision, document));

        // 跨文档粘贴。
        using var other = new Engine(dll);
        other.Paste(other.GetState().Revision, clipboard);
        Require(other.GetState().StrokeCount == 2, "clipboard pastes into another document");
        other.Begin(2, 0);
        other.Append([new() { X = 0, Y = 0, TimeUs = 0 }]);
        Expect(Status.InvalidState, () => other.TransformStrokes(other.GetState().Revision, [0u], Transform.Translate(1, 0)));
        other.Cancel();
    }
}
