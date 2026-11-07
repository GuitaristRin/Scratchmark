// NuGet 消费方冒烟：只用包提供的 API，不传原生库路径（走 runtimes/<rid>/native 解析）。
using Scratchmark;

int checks = 0;
void Require(bool condition, string what)
{
    checks++;
    if (!condition)
    {
        Console.Error.WriteLine($"check failed: {what}");
        Environment.Exit(1);
    }
}

using var engine = new Engine();
// 新文档的初始 revision 是 1（spec/M0 起就如此），这里只验证"空文档"。
Require(engine.GetState().StrokeCount == 0 && engine.GetState().Revision == 1, "fresh document");
// 41 个点：切分后每段应当保留中间顶点（不是只剩首尾的直线弦）。
var samples = new Sample[41];
for (int i = 0; i <= 40; i++)
    samples[i] = new() { X = i * 2, Y = (i % 10) * 3, TimeUs = (ulong)(i * 1000) };
engine.Begin(3, 0x11223344);
engine.Append(samples);
engine.Finish();
State state = engine.GetState();
Require(state.StrokeCount == 1, "one stroke");

StrokeInfo info = engine.GetStroke(state.Revision, 0);
Require(info.PointCount == 41, "point count");

byte[] snapshot = engine.Save();
Require(snapshot.Length > 8 && snapshot[7] == 0, "snapshot format 1");

engine.ErasePath(state.Revision, [new() { X = 40, Y = 0 }], 8);
state = engine.GetState();
Require(state.StrokeCount == 2, "erase splits");

Sample[] first = ReadPoints(engine, state.Revision, 0);
Require((first[^1].Flags & SampleFlags.CutEnd) != 0, "cut end marked");
Require(first.Length > 2, $"fragment keeps interior vertices ({first.Length})");

Bounds bounds = engine.DocumentBounds(state.Revision).Bounds;
Require(bounds.MinX < 0, "bounds include half width");

double ratio = BrushPresets.Ratio(BrushKind.Pencil, 3);
Require(Math.Abs(ratio - 0.26) < 1e-12, "brush preset");

byte[] clipboard = engine.CutStrokes(state.Revision, [0u]);
Require(clipboard.Length > 8, "atomic cut");
state = engine.GetState();
Require(state.StrokeCount == 1, "cut removed one stroke");

Console.WriteLine($"nuget probe: {checks} checks passed");
return 0;

static Sample[] ReadPoints(Engine engine, ulong revision, uint index)
{
    StrokeInfo info = engine.GetStroke(revision, index);
    var buffer = new Sample[Math.Max(1, (int)info.PointCount)];
    int written = (int)engine.ReadPoints(revision, index, 0, buffer);
    return buffer[..written];
}
