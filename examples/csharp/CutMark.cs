using Scratchmark;

internal static partial class Program
{
    private static void CutMark(string dll)
    {
        using var engine = new Engine(dll);
        CommitLine(engine, 0);
        State state = engine.GetState();

        // 先在中间擦一刀，切点应带 CutEnd。
        engine.ErasePath(state.Revision, [new() { X = 5, Y = 0 }], 2.0);
        state = engine.GetState();
        Require(state.StrokeCount == 2, "erase splits the stroke in two");

        Sample[] first = ReadPoints(engine, state.Revision, 0);
        Require((first[0].Flags & SampleFlags.CutEnd) == 0, "original start is not a cut end");
        Require((first[^1].Flags & SampleFlags.CutEnd) != 0, "cut end is marked");

        Sample[] second = ReadPoints(engine, state.Revision, 1);
        Require((second[0].Flags & SampleFlags.CutEnd) != 0, "second fragment starts at a cut end");
        Require((second[^1].Flags & SampleFlags.CutEnd) == 0, "second fragment keeps the original end");

        // 标记随快照往返（仍是格式 1）。
        byte[] snapshot = engine.Save();
        Require(snapshot[7] == 0, "cut marks need no new snapshot format");
        using var reopened = new Engine(dll);
        reopened.Load(snapshot);
        State loaded = reopened.GetState();
        Sample[] restored = ReadPoints(reopened, loaded.Revision, 0);
        Require((restored[^1].Flags & SampleFlags.CutEnd) != 0, "marks survive save and load");

        // 标记不参与命中：切口处擦不到了，两段内部仍在。
        Require(!reopened.HitTest(loaded.Revision, 5, 0, 0).Found, "erased gap is no longer hittable");
        Require(reopened.HitTest(loaded.Revision, 1, 0, 0).Found, "fragment body is still hittable");

        // 写入通道拒绝伪造切点。
        engine.Begin(2, 0);
        Expect(Status.InvalidArgument,
            () => engine.Append([new() { X = 0, Y = 0, TimeUs = 0, Flags = SampleFlags.CutEnd }]));
        engine.Cancel();
    }

    private static Sample[] ReadPoints(Engine engine, ulong revision, uint index)
    {
        StrokeInfo info = engine.GetStroke(revision, index);
        var buffer = new Sample[Math.Max(1, (int)info.PointCount)];
        int written = (int)engine.ReadPoints(revision, index, 0, buffer);
        return buffer[..written];
    }
}
