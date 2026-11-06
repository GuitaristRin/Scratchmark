using System.Runtime.InteropServices;
using Scratchmark;

internal static unsafe partial class Program
{
    private static int checks;
    private static void Require(bool condition, string name)
    {
        Interlocked.Increment(ref checks);
        if (!condition) throw new Exception($"check failed: {name}");
    }

    private static void Expect(Status status, Action action)
    {
        try { action(); }
        catch (ScratchmarkException error) { Require(error.Status == status, status.ToString()); return; }
        throw new Exception($"expected {status}");
    }

    private static void Layout<T>(int size, (string Field, int Offset)[] fields) where T : struct
    {
        Require(Marshal.SizeOf<T>() == size, $"{typeof(T).Name} size");
        foreach (var field in fields)
            Require(Marshal.OffsetOf<T>(field.Field).ToInt64() == field.Offset, field.Field);
    }

    private static int Main(string[] args)
    {
        if (args.Length == 2 && args[0] == "--reject-old-sdk")
        {
            try { using var unexpected = new Engine(args[1]); }
            catch (NotSupportedException) { Console.WriteLine("old SDK rejected before document creation"); return 0; }
            throw new Exception("expected old SDK capability rejection");
        }
        if (args.Length != 3) { Console.Error.WriteLine("usage: host ABS_DLL FIXTURE_HEX OUTPUT_SNAPSHOT"); return 2; }
        Layout<Sample>(32, [("X", 0), ("Y", 8), ("TimeUs", 16), ("Pressure", 24), ("Flags", 28)]);
        Layout<State>(32, [("Revision", 0), ("StrokeCount", 8), ("ActivePoints", 12), ("HasActive", 16),
            ("CanUndo", 20), ("CanRedo", 24), ("Reserved", 28)]);
        Layout<StrokeInfo>(56, [("Width", 0), ("MinX", 8), ("MinY", 16), ("MaxX", 24), ("MaxY", 32),
            ("Color", 40), ("PointCount", 44), ("Active", 48), ("Reserved", 52)]);
        byte[] fixture = File.ReadAllText(args[1]).Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries)
            .Select(s => Convert.ToByte(s, 16)).ToArray();
        Workflow(args[0], fixture, args[2]);
        NativeErrors(fixture);
        Lifecycle(args[0]);
        BrushGeometry(args[0], fixture);
        PageViewport(args[0], fixture);
        HitErase(args[0]);
        LocalErase(args[0]);
        Selection(args[0]);
        Console.WriteLine($"C# smoke: {checks} checks passed");
        return 0;
    }

    private static void Workflow(string dll, byte[] fixture, string outputPath)
    {
        using var engine = new Engine(dll);
        Sample[] samples = [new() { X = 0, Y = 1, TimeUs = 10 },
            new() { X = 2, Y = -1, TimeUs = 20, Pressure = 0.5f, Flags = 1 }];
        Expect(Status.InvalidState, engine.Finish);
        engine.Begin(2, 0x11223344);
        engine.Append(samples);
        State active = engine.GetState();
        Require(active.HasActive == 1 && active.ActivePoints == 2 && active.StrokeCount == 0, "active state");
        StrokeInfo info = engine.GetStroke(active.Revision, 0);
        Require(info.MinX == -1 && info.MinY == -2 && info.MaxX == 3 && info.MaxY == 2, "bounds");
        Require(info.Active == 1 && info.Reserved == 0 && active.Reserved == 0, "flags");
        Expect(Status.InvalidArgument, () => engine.Append([
            new() { X = 3, TimeUs = 30 }, new() { X = double.NaN, TimeUs = 40 }]));
        Require(engine.GetState().Revision == active.Revision && engine.GetState().ActivePoints == 2, "atomic batch");
        engine.Append([]);
        Require(engine.GetState().Revision == active.Revision, "empty batch");
        Expect(Status.InvalidState, () => engine.Save());
        Sample[] read = new Sample[2];
        Require(engine.ReadPoints(active.Revision, 0, 0, read) == 2, "read count");
        Require(read[0].Y == 1 && read[1].X == 2 && read[1].TimeUs == 20 &&
            read[1].Pressure == 0.5f && read[1].Flags == 1, "sample fields");
        Require(engine.ReadPoints(active.Revision, 0, 1, read.AsSpan(0, 1)) == 1 && read[0].X == 2, "partial read");
        Expect(Status.InvalidArgument, () => engine.ReadPoints(active.Revision, 0, 3, read));
        engine.Finish();
        Expect(Status.StaleRevision, () => engine.GetStroke(active.Revision, 0));
        byte[] saved = engine.Save();
        Require(saved.SequenceEqual(fixture), "edit result equals independent fixture");
        File.WriteAllBytes(outputPath, saved);
        using var restored = new Engine(dll);
        restored.Load(fixture);
        Require(restored.Save().SequenceEqual(fixture), "fixture re-encodes exactly");
        State before = restored.GetState();
        byte[] corrupt = (byte[])fixture.Clone(); corrupt[0] ^= 1;
        Expect(Status.InvalidData, () => restored.Load(corrupt));
        Require(restored.GetState().Revision == before.Revision && restored.Save().SequenceEqual(fixture), "failed load preserves document");
        corrupt = (byte[])fixture.Clone(); corrupt[8] = 2;
        Expect(Status.UnsupportedVersion, () => restored.Load(corrupt));
        Require(restored.GetState().Revision == before.Revision, "unsupported version preserves revision");
        restored.Undo();
        Require(restored.GetState().CanRedo == 1 && restored.GetState().StrokeCount == 0, "undo");
        restored.Begin(1, 0); restored.Append(samples.AsSpan(0, 1)); restored.Cancel(); restored.Redo();
        Require(restored.Save().SequenceEqual(fixture), "cancel preserves redo");
        restored.Begin(3, 0x00ff00ff); restored.Append(samples); restored.Finish();
        Require(restored.GetState().StrokeCount == 2, "continue editing restored document");
        Expect(Status.InvalidState, restored.Redo);
    }
}
