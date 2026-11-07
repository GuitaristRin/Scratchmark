using Scratchmark;

internal static partial class Program
{
    private static void StylusAxes(string dll)
    {
        using var engine = new Engine(dll);

        // 第一笔：带轴向。
        engine.Begin(2, 0x11223344);
        Sample[] samples =
        [
            new() { X = 0, Y = 0, TimeUs = 0 },
            new() { X = 10, Y = 0, TimeUs = 1_000 },
        ];
        SampleAxes[] axes = [new() { Tilt = 0, Orientation = 0 }, new() { Tilt = 0.75f, Orientation = -2f }];
        engine.AppendAxes(samples, axes);
        State active = engine.GetState();

        SampleAxes[] read = new SampleAxes[2];
        Require(engine.ReadAxes(active.Revision, 0, 0, read) == 2, "axes read count matches points");
        Require(read[1].Tilt == 0.75f && read[1].Orientation == -2f, "axes round-trip through the binding");
        engine.Finish();

        // 第二笔：普通 append 写入默认轴向 (0, 0)，同时验证非法轴向被整批拒绝。
        engine.Begin(2, 0);
        engine.Append([new() { X = 0, Y = 20, TimeUs = 0 }]);
        State second = engine.GetState();
        SampleAxes[] flat = new SampleAxes[1];
        Require(engine.ReadAxes(second.Revision, 1, 0, flat) == 1 && flat[0].Tilt == 0 && flat[0].Orientation == 0,
            "plain append writes default axes");
        Expect(Status.InvalidArgument, () => engine.AppendAxes(samples.AsSpan(0, 1), [new() { Tilt = 2.0f }]));
        Expect(Status.InvalidArgument, () => engine.AppendAxes(samples.AsSpan(0, 1), [new() { Orientation = 4.0f }]));
        engine.Cancel();

        // 带轴向的文档存成格式 3，重开后轴向仍在。
        byte[] snapshot = engine.Save();
        Require(snapshot.Length > 8 && snapshot[0] == (byte)'S' && snapshot[7] == (byte)'3', "snapshot with axes uses format 3");
        using var reopened = new Engine(dll);
        reopened.Load(snapshot);
        State state = reopened.GetState();
        SampleAxes[] restored = new SampleAxes[2];
        Require(reopened.ReadAxes(state.Revision, 0, 0, restored) == 2, "reopened axes count");
        Require(restored[1].Tilt == 0.75f && restored[1].Orientation == -2f, "reopened axes match");

        // 没有轴向的文档仍是格式 1。
        using var plain = new Engine(dll);
        plain.Begin(2, 0);
        plain.Append([new() { X = 0, Y = 0, TimeUs = 0 }]);
        plain.Finish();
        Require(plain.Save()[7] == 0, "document without axes stays in format 1");

        // 复制粘贴保留轴向（剪贴板是格式 3、页面标识为 0）。
        state = engine.GetState();
        byte[] clipboard = engine.CopyStrokes(state.Revision, [0u]);
        Require(clipboard[7] == (byte)'3', "clipboard with axes uses format 3");
        engine.Paste(state.Revision, clipboard);
        state = engine.GetState();
        SampleAxes[] pasted = new SampleAxes[2];
        Require(engine.ReadAxes(state.Revision, 1, 0, pasted) == 2 && pasted[1].Tilt == 0.75f,
            "paste carries axes");
    }
}
