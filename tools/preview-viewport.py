"""用真实 SDK .so 的输出生成缩放/平移前后对照图与量测。

离线合成页面上的计算参考图：验证页面坐标经视口变换后仍与页面内容
对应、输入路径（视口->页面）与显示路径（页面->视口）一致。不涉及
实机手感、设备延迟或显示性能。
"""
import argparse
import ctypes as c
import hashlib
import json
import math
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

candidates = list(Path("/usr/share/fonts").glob("**/DejaVuSans.ttf"))
if not candidates:
    raise RuntimeError("需要 DejaVuSans.ttf 作为标注字体")
font_path = str(candidates[0])
FONT = ImageFont.truetype(font_path, 14)
FONT_SMALL = ImageFont.truetype(font_path, 12)


class Sample(c.Structure):
    _fields_ = [("x", c.c_double), ("y", c.c_double), ("time", c.c_uint64),
                ("pressure", c.c_float), ("flags", c.c_uint32)]


class Point(c.Structure):
    _fields_ = [("x", c.c_double), ("y", c.c_double)]


class Bounds(c.Structure):
    _fields_ = [("min_x", c.c_double), ("min_y", c.c_double),
                ("max_x", c.c_double), ("max_y", c.c_double)]


class Viewport(c.Structure):
    _fields_ = [("scale", c.c_double), ("offset_x", c.c_double),
                ("offset_y", c.c_double), ("reserved", c.c_uint32)]


class BrushPoint(c.Structure):
    _fields_ = [("x", c.c_double), ("y", c.c_double), ("width", c.c_double)]


class BrushOptions(c.Structure):
    _fields_ = [("alpha", c.c_double), ("ratio", c.c_double),
                ("pressure", c.c_uint32), ("reserved", c.c_uint32)]


class State(c.Structure):
    _fields_ = [("revision", c.c_uint64)] + [(k, c.c_uint32) for k in
                ("strokes", "active_points", "active", "undo", "redo", "reserved")]


def bind(lib):
    lib.sm_create.argtypes = [c.POINTER(c.c_uint64)]
    lib.sm_destroy.argtypes = [c.c_uint64]
    lib.sm_begin.argtypes = [c.c_uint64, c.c_double, c.c_uint32]
    lib.sm_append.argtypes = [c.c_uint64, c.POINTER(Sample), c.c_uint32]
    lib.sm_finish.argtypes = [c.c_uint64]
    lib.sm_read_points.argtypes = [c.c_uint64, c.c_uint64, c.c_uint32, c.c_uint32,
        c.POINTER(Sample), c.c_uint32, c.POINTER(c.c_uint32)]
    lib.sm_get_state.argtypes = [c.c_uint64, c.POINTER(State)]
    lib.sm_set_page_id.argtypes = [c.c_uint64, c.c_uint64]
    lib.sm_get_page_id.argtypes = [c.c_uint64, c.POINTER(c.c_uint64)]
    lib.sm_read_brush_points.argtypes = [c.c_uint64, c.c_uint64, c.c_uint32,
        c.c_uint32, c.POINTER(BrushOptions), c.POINTER(BrushPoint),
        c.c_uint32, c.POINTER(c.c_uint32)]
    lib.sm_save.argtypes = [c.c_uint64, c.POINTER(c.c_uint8), c.c_uint32,
        c.POINTER(c.c_uint32)]
    lib.sm_load.argtypes = [c.c_uint64, c.POINTER(c.c_uint8), c.c_uint32]
    lib.sm_page_to_viewport.argtypes = [c.POINTER(Viewport), c.POINTER(Point),
        c.POINTER(Point)]
    lib.sm_viewport_to_page.argtypes = [c.POINTER(Viewport), c.POINTER(Point),
        c.POINTER(Point)]
    lib.sm_width_page_to_viewport.argtypes = [c.POINTER(Viewport),
        c.c_double, c.POINTER(c.c_double)]
    lib.sm_features.restype = c.c_uint64


def check(status, what):
    if status != 0:
        raise RuntimeError(f"{what}: status {status}")


def to_viewport(lib, view, x, y):
    out = Point()
    check(lib.sm_page_to_viewport(c.byref(view), c.byref(Point(x, y)),
        c.byref(out)), "page_to_viewport")
    return out.x, out.y


def to_page(lib, view, x, y):
    out = Point()
    check(lib.sm_viewport_to_page(c.byref(view), c.byref(Point(x, y)),
        c.byref(out)), "viewport_to_page")
    return out.x, out.y


def page_width(lib, view, width):
    out = c.c_double()
    check(lib.sm_width_page_to_viewport(c.byref(view), width, c.byref(out)),
        "width_page_to_viewport")
    return out.value


def draw_stroke(draw, segments, color):
    """segments: [(x0,y0,w0,x1,y1,w1)] 页面/视口坐标 + 完整笔宽，按线性半径插值扫掠圆盘。"""
    for x0, y0, w0, x1, y1, w1 in segments:
        length = math.hypot(x1 - x0, y1 - y0)
        steps = max(1, int(length * 2))
        for i in range(steps + 1):
            t = i / steps
            x = x0 + (x1 - x0) * t
            y = y0 + (y1 - y0) * t
            w = (w0 + (w1 - w0) * t) / 2.0
            draw.ellipse((x - w, y - w, x + w, y + w), fill=color)


def render_panel(page_rect, grid, stroke_pts, view, marker, overlay_dots, size, label):
    """把页面内容经真实 .so 变换到视口，再用等比仿射适配画布绘制。

    所有元素（网格、边界、笔划、回放点、标记）都走同一个视口->像素
    映射，保证图上对齐关系就是变换的数学对齐关系。
    """
    supersample = 2
    canvas_px = size * supersample
    img = Image.new("RGB", (canvas_px, canvas_px), "white")
    draw = ImageDraw.Draw(img)

    min_x, min_y, max_x, max_y = page_rect
    corners = [to_viewport(lib, view, min_x, min_y), to_viewport(lib, view, max_x, max_y)]
    vp_min_x = min(p[0] for p in corners)
    vp_max_x = max(p[0] for p in corners)
    vp_min_y = min(p[1] for p in corners)
    vp_max_y = max(p[1] for p in corners)
    pad = 24 * supersample
    fit = min((canvas_px - 2 * pad) / (vp_max_x - vp_min_x),
              (canvas_px - 2 * pad) / (vp_max_y - vp_min_y))
    # 面板内容在画布内水平居中。
    base_x = (canvas_px - (vp_max_x - vp_min_x) * fit) / 2.0
    base_y = (canvas_px - (vp_max_y - vp_min_y) * fit) / 2.0

    def px(x, y):
        vx, vy = to_viewport(lib, view, x, y)
        return (base_x + (vx - vp_min_x) * fit, base_y + (vy - vp_min_y) * fit)

    def pxv(vx, vy):
        # overlay 点已经是视口坐标，直接映射到像素。
        return (base_x + (vx - vp_min_x) * fit, base_y + (vy - vp_min_y) * fit)

    for gx in range(int(min_x), int(max_x) + 1, grid):
        x0, y0 = px(gx, min_y)
        x1, y1 = px(gx, max_y)
        draw.line((x0, y0, x1, y1), fill=(225, 228, 235), width=supersample)
    for gy in range(int(min_y), int(max_y) + 1, grid):
        x0, y0 = px(min_x, gy)
        x1, y1 = px(max_x, gy)
        draw.line((x0, y0, x1, y1), fill=(225, 228, 235), width=supersample)
    bx0, by0 = px(min_x, min_y)
    bx1, by1 = px(max_x, max_y)
    draw.rectangle((bx0, by0, bx1, by1), outline=(150, 156, 168), width=2 * supersample)

    segments = []
    for (x0, y0, w0), (x1, y1, w1) in zip(stroke_pts, stroke_pts[1:]):
        p0 = px(x0, y0)
        p1 = px(x1, y1)
        segments.append((p0[0], p0[1], page_width(lib, view, w0) * fit,
                         p1[0], p1[1], page_width(lib, view, w1) * fit))
    draw_stroke(draw, segments, (43, 87, 154))

    for dx, dy in overlay_dots:
        ox, oy = pxv(dx, dy)
        r = 2.5 * supersample
        draw.ellipse((ox - r, oy - r, ox + r, oy + r), fill=(204, 61, 57))

    mx, my = px(marker[0], marker[1])
    arm = 8 * supersample
    draw.line((mx - arm, my, mx + arm, my), fill=(20, 20, 20), width=supersample)
    draw.line((mx, my - arm, mx, my + arm), fill=(20, 20, 20), width=supersample)

    img = img.resize((size, size), Image.LANCZOS)
    draw = ImageDraw.Draw(img)
    draw.text((8, 6), label, fill=(30, 30, 30), font=FONT)
    vx, vy = to_viewport(lib, view, marker[0], marker[1])
    draw.text((8, size - 34),
              f"marker page ({marker[0]:.4f}, {marker[1]:.4f})", fill=(30, 30, 30),
              font=FONT_SMALL)
    draw.text((8, size - 18),
              f"-> viewport ({vx:.4f}, {vy:.4f}) scale={view.scale:g} offset=({view.offset_x:g},{view.offset_y:g})",
              fill=(30, 30, 30), font=FONT_SMALL)
    return img


parser = argparse.ArgumentParser()
parser.add_argument("so", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
lib = c.CDLL(str(args.so.resolve()))
bind(lib)
if not lib.sm_features() & 0b10:
    raise RuntimeError("SDK lacks page/viewport feature bit")

# 页面坐标里的合成笔划；样本只保存在引擎内，变换全部经真实 .so 计算。
page_rect = (10.0, 10.0, 210.0, 160.0)
grid = 25
samples = []
for i in range(160):
    t = i / 159
    x = page_rect[0] + t * (page_rect[2] - page_rect[0] - 20.0)
    y = 85.0 + 45.0 * math.sin(t * 2.6)
    samples.append(Sample(x, y, i * 800, 0.25 + 0.65 * abs(math.sin(t * 3.1)), 1))

handle = c.c_uint64()
check(lib.sm_create(c.byref(handle)), "create")
check(lib.sm_set_page_id(handle, 0x20261104), "set_page_id")
check(lib.sm_begin(handle, 5.0, 0x2C579AFF), "begin")
check(lib.sm_append(handle, (Sample * len(samples))(*samples), len(samples)), "append")
check(lib.sm_finish(handle), "finish")

required = c.c_uint32()
# 规格约定：NULL / 0 查询大小返回 BUFFER_TOO_SMALL 并填写 required。
status = lib.sm_save(handle, None, 0, c.byref(required))
if status != 4:
    raise RuntimeError(f"save size query: status {status}")
buffer = (c.c_uint8 * required.value)()
check(lib.sm_save(handle, buffer, required, c.byref(required)), "save")
snapshot = bytes(buffer)
magic_ok = snapshot[:8] == b"SCRATCH2"

reopened = c.c_uint64()
check(lib.sm_create(c.byref(reopened)), "create reopen")
check(lib.sm_load(reopened, (c.c_uint8 * len(snapshot)).from_buffer_copy(snapshot),
                  len(snapshot)), "load")
page_id = c.c_uint64()
check(lib.sm_get_page_id(reopened, c.byref(page_id)), "get_page_id")

state = State()
check(lib.sm_get_state(reopened, c.byref(state)), "get_state")
options = BrushOptions(0.5, 0.25, 1, 0)
# 已提交笔划的读取索引为 stroke_count - 1（本轮只有一笔）。
index = state.strokes - 1
info_count = len(samples)
points = (BrushPoint * info_count)()
written = c.c_uint32()
check(lib.sm_read_brush_points(reopened, state.revision, index, 0,
    c.byref(options), points, info_count, c.byref(written)), "read_brush_points")
stroke_pts = [(p.x, p.y, p.width) for p in points[:written.value]]

# 输入路径：设备点取自变换后的采样（缩放平移后的屏幕位置），
# 经 sm_viewport_to_page 回到页面域再提交，验证仍落在同一轨迹上。
view_before = Viewport(1.0, 0.0, 0.0, 0)
view_after = Viewport(2.0, 60.0, 40.0, 0)
device_hits = []
replay_lib_points = []
for i in range(0, len(samples), 12):
    vx, vy = to_viewport(lib, view_after, samples[i].x, samples[i].y)
    device_hits.append((vx, vy))
    px, py = to_page(lib, view_after, vx, vy)
    replay_lib_points.append((samples[i].x, samples[i].y, px, py))

replay = c.c_uint64()
check(lib.sm_create(c.byref(replay)), "create replay")
check(lib.sm_set_page_id(replay, page_id.value), "replay page id")
check(lib.sm_begin(replay, 5.0, 0x2C579AFF), "replay begin")
replay_samples = [Sample(px, py, i * 800, s.pressure, s.flags)
                  for i, ((ox, oy, px, py), s) in enumerate(zip(replay_lib_points, samples[::12]))]
check(lib.sm_append(replay, (Sample * len(replay_samples))(*replay_samples),
                    len(replay_samples)), "replay append")
check(lib.sm_finish(replay), "replay finish")
# 读回原始样本（非派生笔刷）：它们是逆变换后真正提交进引擎的页面坐标。
rstate = State()
check(lib.sm_get_state(replay, c.byref(rstate)), "replay state")
raw_replay = (Sample * len(replay_samples))()
rwritten = c.c_uint32()
check(lib.sm_read_points(replay, rstate.revision, 0, 0, raw_replay,
    len(replay_samples), c.byref(rwritten)), "replay read points")

# 逐点比较：回放原始样本经视口呈现后与原始设备输入的偏差。
replayed_viewport = []
for raw in raw_replay[:rwritten.value]:
    ex, ey = to_viewport(lib, view_after, raw.x, raw.y)
    replayed_viewport.append((ex, ey))
errors = [math.hypot(ex - vx, ey - vy)
          for (ex, ey), (vx, vy) in zip(replayed_viewport, device_hits)]
max_replay_error = max(errors)
roundtrip_errors = []
for x, y, _ in stroke_pts:
    vx, vy = to_viewport(lib, view_after, x, y)
    bx, by = to_page(lib, view_after, vx, vy)
    roundtrip_errors.append(math.hypot(bx - x, by - y))

marker = stroke_pts[80]
size = 500
panel_a = render_panel(page_rect, grid, stroke_pts, view_before, marker, [], size,
                       "before: page space, scale=1 offset=(0,0)")
panel_b = render_panel(page_rect, grid, stroke_pts, view_after, marker,
                       replayed_viewport, size,
                       "after: scale=2 offset=(60,40), red = replayed device input")
gap = 12
canvas = Image.new("RGB", (size * 2 + gap * 3, size + 56), "white")
canvas.paste(panel_a, (gap, 8))
canvas.paste(panel_b, (size + gap * 2, 8))
draw = ImageDraw.Draw(canvas)
draw.text((gap, size + 16),
          "left: viewport == page (identity). right: same page content under zoom/pan; "
          "grid and stroke stay aligned.", fill=(60, 60, 60), font=FONT_SMALL)
draw.text((gap, size + 34),
          "red dots: raw samples appended through sm_viewport_to_page, re-shown in the same viewport "
          f"(max deviation {max_replay_error:.3e} px).", fill=(60, 60, 60), font=FONT_SMALL)
png_path = args.output / "viewport-preview.png"
canvas.save(png_path)

metrics = {
    "so_sha256": hashlib.sha256(args.so.read_bytes()).hexdigest(),
    "page_id": page_id.value,
    "snapshot_magic": "SCRATCH2" if magic_ok else snapshot[:8].hex(),
    "snapshot_bytes": len(snapshot),
    "viewports": {
        "before": {"scale": 1.0, "offset": [0.0, 0.0]},
        "after": {"scale": 2.0, "offset": [60.0, 40.0]},
    },
    "brush_options": {"alpha": 0.5, "min_width_ratio": 0.25, "pressure_enabled": 1},
    "brush_points": written.value,
    "width_range_page": [min(p[2] for p in stroke_pts), max(p[2] for p in stroke_pts)],
    "width_range_after_viewport": [
        page_width(lib, view_after, min(p[2] for p in stroke_pts)),
        page_width(lib, view_after, max(p[2] for p in stroke_pts)),
    ],
    "marker": {"page": [marker[0], marker[1]],
               "viewport_after": list(to_viewport(lib, view_after, marker[0], marker[1]))},
    "roundtrip_max_error_page_units": max(roundtrip_errors),
    "replayed_raw_samples_max_deviation_viewport_px": max_replay_error,
    "note": "offline composite page from real .so output; not a device-feel or latency result",
}
(args.output / "metrics.json").write_text(json.dumps(metrics, indent=2) + "\n")

lib.sm_destroy(handle)
lib.sm_destroy(reopened)
lib.sm_destroy(replay)
print(png_path)
print(json.dumps({k: metrics[k] for k in ("roundtrip_max_error_page_units",
    "replayed_raw_samples_max_deviation_viewport_px", "snapshot_magic")}, indent=2))
