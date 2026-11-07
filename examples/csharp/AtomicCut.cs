using Scratchmark;

internal static partial class Program
{
    private static void AtomicCut(string dll)
    {
        using var engine = new Engine(dll);
        CommitLine(engine, 0);
        CommitLine(engine, 100);
        State before = engine.GetState();
        Require(before.StrokeCount == 2, "two strokes before cut");

        // 查询模式：拿到长度但不剪切。
        byte[] clipboard = engine.CutStrokes(before.Revision, [0u]);
        Require(clipboard.Length > 8, "cut returns clipboard bytes");
        State after = engine.GetState();
        Require(after.StrokeCount == 1, "cut removes the selection");
        Require(after.Revision == before.Revision + 1, "cut bumps the revision once");

        // 一次撤销整体恢复。
        engine.Undo();
        State restored = engine.GetState();
        Require(restored.StrokeCount == 2, "one undo restores the cut stroke");
        // CommitLine 把线画在 x 上：第二笔是 x=100..110。
        Require(Math.Abs(engine.GetStroke(restored.Revision, 1).MinX - 99) < 1e-9, "restored stroke is at its old place");

        // 剪贴板内容能粘贴回原几何。
        engine.Paste(restored.Revision, clipboard);
        State pasted = engine.GetState();
        Require(pasted.StrokeCount == 3, "clipboard pastes back");
        Require(Math.Abs(engine.GetStroke(pasted.Revision, 2).MinX - (-1)) < 1e-9, "pasted stroke keeps its geometry");

        // 非法输入不改文档。
        State guard = engine.GetState();
        Expect(Status.InvalidArgument, () => engine.CutStrokes(guard.Revision, []));
        Expect(Status.InvalidArgument, () => engine.CutStrokes(guard.Revision, [0u, 0u]));
        Expect(Status.InvalidArgument, () => engine.CutStrokes(guard.Revision, [99u]));
        Expect(Status.StaleRevision, () => engine.CutStrokes(guard.Revision - 1, [0u]));
        Require(engine.GetState().StrokeCount == guard.StrokeCount, "failed cuts leave the document alone");
    }
}
