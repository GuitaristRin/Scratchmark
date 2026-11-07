using Scratchmark;
using Point = Scratchmark.Point;

internal static partial class Program
{
    private static void ViewportRotation(string dll)
    {
        using var engine = new Engine(dll);
        CommitLine(engine, 0);

        var flat = new ViewportEx { Scale = 1 };
        var quarter = new ViewportEx { Scale = 1, Angle = Math.PI / 2 };
        Point page = new() { X = 10, Y = 0 };
        Point turned = quarter.PageToViewport(page);
        Require(Math.Abs(turned.X) < 1e-9 && Math.Abs(turned.Y - 10) < 1e-9, "90 degrees maps (10,0) to (0,10)");
        Point back = quarter.ViewportToPage(turned);
        Require(Math.Abs(back.X - page.X) < 1e-9 && Math.Abs(back.Y - page.Y) < 1e-9, "rotation round-trips");

        // 旧结构等价于 angle = 0。
        var legacy = new Viewport { Scale = 2.5, OffsetX = 13, OffsetY = -7 };
        Point legacyPoint = legacy.PageToViewport(new Point { X = 12, Y = -4 });
        Point exPoint = new ViewportEx { Scale = 2.5, OffsetX = 13, OffsetY = -7 }.PageToViewport(new Point { X = 12, Y = -4 });
        Require(legacyPoint.X == exPoint.X && legacyPoint.Y == exPoint.Y, "zero angle matches the legacy viewport");

        // 旋转不改变线宽换算，只由 scale 决定。
        Require(Math.Abs(quarter.WidthPageToViewport(6) - 6) < 1e-9, "rotation does not scale width");
        Require(Math.Abs(new ViewportEx { Scale = 2, Angle = 1 }.WidthPageToViewport(6) - 12) < 1e-9, "scale drives width");

        // 外接框：90° 时宽高互换。
        Bounds boxed = quarter.BoundsPageToViewport(new Bounds { MinX = 0, MinY = 0, MaxX = 100, MaxY = 20 });
        Require(Math.Abs(boxed.MaxX - boxed.MinX - 20) < 1e-9, "rotated bounds swap width and height");

        // 以光标为锚缩放：锚点下的页面点不动。
        var vp = new ViewportEx { Scale = 1.3, Angle = 0.6, OffsetX = 12, OffsetY = -8 };
        Point anchor = new() { X = 240, Y = 180 };
        Point before = vp.ViewportToPage(anchor);
        ViewportEx zoomed = vp.ZoomAt(anchor, 1.8);
        Point after = zoomed.ViewportToPage(anchor);
        Require(Math.Abs(after.X - before.X) < 1e-9 && Math.Abs(after.Y - before.Y) < 1e-9, "zoom keeps the page point under the anchor");
        Require(Math.Abs(zoomed.Scale - 1.3 * 1.8) < 1e-9, "zoom multiplies scale");

        // 适应窗口：页面中心落在视口中心。
        ViewportEx fitted = ViewportEx.Fit(new Bounds { MinX = 0, MinY = 0, MaxX = 800, MaxY = 600 }, 1000, 800, Math.PI / 2, 20);
        Point center = fitted.PageToViewport(new Point { X = 400, Y = 300 });
        Require(Math.Abs(center.X - 500) < 1e-9 && Math.Abs(center.Y - 400) < 1e-9, "fit centers the page");
        Require(Math.Abs(fitted.Scale - 760.0 / 800.0) < 1e-9, "fit uses the rotated bounding box");

        // 非法输入被拒绝。
        Expect(Status.InvalidArgument, () => new ViewportEx { Scale = 0 }.PageToViewport(page));
        Expect(Status.InvalidArgument, () => new ViewportEx { Scale = 1, Angle = double.NaN }.PageToViewport(page));
        Expect(Status.InvalidArgument, () => vp.ZoomAt(anchor, 0));
        Expect(Status.InvalidArgument, () => ViewportEx.Fit(new Bounds { MinX = 0, MinY = 0, MaxX = 10, MaxY = 10 }, 100, 100, 0, 60));

        // 同一份视口同时用于显示与输入时，笔迹与页面内容对得上。
        State state = engine.GetState();
        StrokeInfo info = engine.GetStroke(state.Revision, 0);
        Point strokeStart = new() { X = info.MinX, Y = info.MinY };
        Point viewA = fitted.PageToViewport(strokeStart);
        Point pageAgain = fitted.ViewportToPage(viewA);
        Require(Math.Abs(pageAgain.X - strokeStart.X) < 1e-9, "fit viewport round-trips stroke geometry");
        Require(Math.Abs(flat.Scale - 1) < 1e-12, "identity viewport still usable");
    }
}
