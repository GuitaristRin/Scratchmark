using Scratchmark;

internal static unsafe partial class Program
{
    private static void BrushGeometry(string dll, byte[] fixture)
    {
        Layout<BrushOptions>(24, [("Alpha", 0), ("MinWidthRatio", 8), ("PressureEnabled", 16), ("Reserved", 20)]);
        Layout<BrushPoint>(24, [("X", 0), ("Y", 8), ("Width", 16)]);
        Require((Native.Features() & 1) == 1, "brush feature");
        using var engine = new Engine(dll);
        engine.Load(fixture);
        var options = new BrushOptions { Alpha = 0.5, MinWidthRatio = 0.25, PressureEnabled = 1 };
        var output = new BrushPoint[2];
        ulong revision = engine.GetState().Revision;
        Require(engine.ReadBrushPoints(revision, 0, 0, options, output) == 2, "brush read");
        Require(output[0].X == 0 && output[0].Y == 1 && output[0].Width == 2, "missing pressure fallback");
        Require(output[1].X == 1 && output[1].Y == 0 && output[1].Width == 1.25, "hand calculated brush");
        Require(engine.GetState().Revision == revision && engine.Save().SequenceEqual(fixture), "derived cache does not mutate raw snapshot");
        output[0] = new BrushPoint { X = 99, Y = 98, Width = 97 };
        Expect(Status.StaleRevision, () => engine.ReadBrushPoints(revision - 1, 0, 0, options, output));
        Require(output[0].X == 99 && output[0].Width == 97, "stale brush output preserved");
        var invalid = options; invalid.Alpha = 0;
        Expect(Status.InvalidArgument, () => engine.ReadBrushPoints(revision, 0, 0, invalid, output));
        Require(output[0].X == 99, "bad brush output preserved");
        var identity = new BrushOptions { Alpha = 1, MinWidthRatio = 1 };
        engine.ReadBrushPoints(revision, 0, 0, identity, output);
        Require(output[1].X == 2 && output[1].Y == -1 && output[1].Width == 2, "disabled brush is identity");
        engine.Begin(10, 0);
        engine.Append([new Sample { X = 0, Y = 0, Flags = 1 }]);
        revision = engine.GetState().Revision;
        engine.ReadBrushPoints(revision, 1, 0, options, output);
        Require(output[0].Width == 2.5, "active pressure dot");
        engine.Append([new Sample { X = 8, Y = 4, TimeUs = 1, Pressure = 1, Flags = 1 }]);
        revision = engine.GetState().Revision;
        engine.ReadBrushPoints(revision, 1, 1, options, output.AsSpan(0, 1));
        Require(output[0].X == 4 && output[0].Y == 2 && output[0].Width == 10, "incremental cached tail");
        engine.Finish();
        byte[] saved = engine.Save();
        using var restored = new Engine(dll); restored.Load(saved);
        restored.ReadBrushPoints(restored.GetState().Revision, 1, 1, options, output);
        Require(output[0].X == 4 && output[0].Y == 2 && output[0].Width == 10, "same configuration restores brush");
    }
}
