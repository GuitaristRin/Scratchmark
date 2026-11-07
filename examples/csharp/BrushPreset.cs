using Scratchmark;

internal static partial class Program
{
    private static void BrushPreset(string dll)
    {
        // 预置表是无句柄纯函数，但仍要通过一次加载核对能力位。
        using var engine = new Engine(dll);

        // 五个笔型各自的五档必须严格递增且落在 (0, 1]。
        foreach (BrushKind kind in Enum.GetValues<BrushKind>())
        {
            double previous = 0;
            for (uint level = 1; level <= BrushPresets.Levels; level++)
            {
                double ratio = BrushPresets.Ratio(kind, level);
                Require(ratio > 0 && ratio <= 1, $"{kind} level {level} ratio in range");
                Require(ratio > previous, $"{kind} level {level} ratio increases");
                previous = ratio;
            }
        }

        // 手算锚点与宽度换算。
        Require(Math.Abs(BrushPresets.Ratio(BrushKind.Pencil, 3) - 0.26) < 1e-12, "pencil level 3 is 0.26");
        Require(Math.Abs(BrushPresets.Width(BrushKind.Pencil, 3, 10) - 2.6) < 1e-12, "pencil level 3 width at 10");

        // 非法输入被拒绝。
        Expect(Status.InvalidArgument, () => BrushPresets.Ratio((BrushKind)0, 1));
        Expect(Status.InvalidArgument, () => BrushPresets.Ratio(BrushKind.Pencil, 0));
        Expect(Status.InvalidArgument, () => BrushPresets.Ratio(BrushKind.Pencil, 6));
        Expect(Status.InvalidArgument, () => BrushPresets.Width(BrushKind.Pen, 1, 0));
        Expect(Status.InvalidArgument, () => BrushPresets.Width(BrushKind.Pen, 1, 1e6 + 1));

        // 预置值可以直接落到引擎的笔宽上：写一笔并读回。
        double width = BrushPresets.Width(BrushKind.Marker, 4, 20);
        engine.Begin(width, 0x11223344);
        engine.Append([new() { X = 0, Y = 0, TimeUs = 0 }, new() { X = 10, Y = 0, TimeUs = 10 }]);
        engine.Finish();
        State state = engine.GetState();
        Require(Math.Abs(engine.GetStroke(state.Revision, 0).Width - width) < 1e-12, "preset width lands on the stroke");
    }
}
