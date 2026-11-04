using Scratchmark;

internal static partial class Program
{
    private static void PageViewport(string dll, byte[] format1Fixture)
    {
        Layout<Viewport>(32, [("Scale", 0), ("OffsetX", 8), ("OffsetY", 16), ("Reserved", 24)]);
        Layout<Point>(16, [("X", 0), ("Y", 8)]);
        Layout<Bounds>(32, [("MinX", 0), ("MinY", 8), ("MaxX", 16), ("MaxY", 24)]);
        using var engine = new Engine(dll);
        Require(engine.GetPageId() == 0, "page id defaults to unset");
        ulong before = engine.GetState().Revision;
        engine.SetPageId(42);
        Require(engine.GetPageId() == 42 && engine.GetState().Revision == before + 1, "set page id bumps revision");
        engine.SetPageId(42);
        Require(engine.GetState().Revision == before + 1, "same page id keeps revision");

        Viewport view = new() { Scale = 2.0, OffsetX = 10.0, OffsetY = 20.0 };
        Point shown = view.PageToViewport(new Point { X = 3, Y = 4 });
        Require(shown.X == 16 && shown.Y == 28, "page to viewport");
        Point back = view.ViewportToPage(shown);
        Require(back.X == 3 && back.Y == 4, "viewport to page");
        Require(view.WidthPageToViewport(5) == 10 && view.WidthViewportToPage(10) == 5, "width scaling");
        Bounds bounds = view.BoundsPageToViewport(new Bounds { MinX = -1, MinY = -2, MaxX = 3, MaxY = 4 });
        Require(bounds.MinX == 8 && bounds.MinY == 16 && bounds.MaxX == 16 && bounds.MaxY == 28, "bounds mapping");
        Expect(Status.InvalidArgument, () => view.PageToViewport(new Point { X = double.NaN, Y = 0 }));
        Expect(Status.InvalidArgument, () => view.PageToViewport(new Point { X = 2e9, Y = 0 }));
        Expect(Status.InvalidArgument, () => view.WidthViewportToPage(0));

        // 输入路径与显示路径在同一视口下几何一致。
        Sample[] page = [new() { X = 0, Y = 0, TimeUs = 0 }, new() { X = 100, Y = 0, TimeUs = 1 },
            new() { X = 100, Y = 40, TimeUs = 2 }];
        engine.Begin(6, 0);
        engine.Append(page);
        State state = engine.GetState();
        BrushPoint[] derived = new BrushPoint[page.Length];
        Require(engine.ReadBrushPoints(state.Revision, 0, 0, new BrushOptions
        {
            Alpha = 0.5,
            MinWidthRatio = 0.25,
            PressureEnabled = 1
        }, derived) == page.Length, "brush read");
        engine.Finish();
        foreach (BrushPoint point in derived)
        {
            Point device = view.PageToViewport(new Point { X = point.X, Y = point.Y });
            Point replay = view.ViewportToPage(device);
            Require(Math.Abs(replay.X - point.X) <= 1e-9 * Math.Max(1, Math.Abs(point.X)) &&
                Math.Abs(replay.Y - point.Y) <= 1e-9 * Math.Max(1, Math.Abs(point.Y)),
                "input and display paths agree");
        }

        // 设置页面标识后快照为格式 2，跨页标识冲突拒绝，未设置标识的引擎整份恢复。
        byte[] snapshot = engine.Save();
        Require(snapshot.AsSpan(0, 8).SequenceEqual("SCRATCH2"u8), "snapshot carries page reference");
        using var conflicting = new Engine(dll);
        conflicting.SetPageId(43);
        Expect(Status.InvalidData, () => conflicting.Load(snapshot));
        Require(conflicting.GetPageId() == 43 && conflicting.GetState().StrokeCount == 0, "conflict keeps document");
        using var adopting = new Engine(dll);
        adopting.Load(snapshot);
        Require(adopting.GetPageId() == 42 && adopting.GetState().StrokeCount == 1, "unset engine adopts page id");
        // 格式 1 快照页面标识视为 0：允许载入并清空当前标识。
        using var legacy = new Engine(dll);
        legacy.SetPageId(7);
        legacy.Load(format1Fixture);
        Require(legacy.GetPageId() == 0 && legacy.GetState().StrokeCount == 1, "format 1 clears page identity");
    }
}
