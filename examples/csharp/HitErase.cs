using Scratchmark;

internal static partial class Program
{
    private static void CommitLine(Engine engine, double x)
    {
        engine.Begin(2, 0);
        engine.Append([new() { X = x, Y = 0, TimeUs = 0 }, new() { X = x + 10, Y = 0, TimeUs = 1 }]);
        engine.Finish();
    }

    private static void HitErase(string dll)
    {
        using var engine = new Engine(dll);
        CommitLine(engine, 0);
        CommitLine(engine, 100);
        State state = engine.GetState();

        HitResult hit = engine.HitTest(state.Revision, 5, 0, 0);
        Require(hit.Found && hit.Index == 0, "hit test finds stroke");
        HitResult miss = engine.HitTest(state.Revision, 1000, 0, 0);
        Require(!miss.Found && miss.Index == 0, "hit test miss");
        Expect(Status.StaleRevision, () => engine.HitTest(state.Revision - 1, 5, 0, 0));
        Expect(Status.InvalidArgument, () => engine.HitTest(state.Revision, 5, 0, -1));

        Expect(Status.StaleRevision, () => engine.EraseStroke(state.Revision - 1, 0));
        Expect(Status.InvalidArgument, () => engine.EraseStroke(state.Revision, 2));

        engine.Begin(2, 0);
        engine.Append([new() { X = 0, Y = 10, TimeUs = 0 }]);
        state = engine.GetState();
        Expect(Status.InvalidState, () => engine.EraseStroke(state.Revision, 0));
        engine.Cancel();

        state = engine.GetState();
        engine.EraseStroke(state.Revision, 0);
        state = engine.GetState();
        Require(state.StrokeCount == 1, "erase removes one stroke");
        Require(!engine.HitTest(state.Revision, 5, 0, 0).Found, "erased stroke is gone");

        engine.Undo();
        state = engine.GetState();
        Require(state.StrokeCount == 2, "undo restores erased stroke");
        HitResult restored = engine.HitTest(state.Revision, 5, 0, 0);
        Require(restored.Found && restored.Index == 0, "undo restores z order");

        engine.Redo();
        state = engine.GetState();
        Require(state.StrokeCount == 1 && !engine.HitTest(state.Revision, 5, 0, 0).Found, "redo erases again");
    }
}
