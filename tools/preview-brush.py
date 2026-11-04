"""读取真实 SDK DLL 的派生点，生成离线参考图与量测，不模拟设备输入延迟。"""
import argparse
import csv
import ctypes as c
import hashlib
import json
import math
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import Circle


class Sample(c.Structure):
    _fields_ = [("x", c.c_double), ("y", c.c_double), ("time", c.c_uint64),
                ("pressure", c.c_float), ("flags", c.c_uint32)]


class Point(c.Structure):
    _fields_ = [("x", c.c_double), ("y", c.c_double), ("width", c.c_double)]


class State(c.Structure):
    _fields_ = [("revision", c.c_uint64)] + [(key, c.c_uint32) for key in
                ("strokes", "active_points", "active", "undo", "redo", "reserved")]


class Options(c.Structure):
    _fields_ = [("alpha", c.c_double), ("ratio", c.c_double),
                ("pressure", c.c_uint32), ("reserved", c.c_uint32)]


parser = argparse.ArgumentParser()
parser.add_argument("dll", type=Path)
parser.add_argument("output", type=Path)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
lib = c.CDLL(str(args.dll.resolve()))
u64, u32 = c.c_uint64, c.c_uint32
signatures = {
    "sm_create": [c.POINTER(u64)], "sm_destroy": [u64],
    "sm_begin": [u64, c.c_double, u32], "sm_finish": [u64],
    "sm_append": [u64, c.POINTER(Sample), u32], "sm_get_state": [u64, c.POINTER(State)],
    "sm_read_brush_points": [u64, u64, u32, u32, c.POINTER(Options), c.POINTER(Point), u32, c.POINTER(u32)],
    "sm_save": [u64, c.POINTER(c.c_ubyte), u32, c.POINTER(u32)],
    "sm_load": [u64, c.POINTER(c.c_ubyte), u32],
}
for name, signature in signatures.items():
    fn = getattr(lib, name)
    fn.argtypes, fn.restype = signature, u32


def ok(status):
    if status != 0:
        raise RuntimeError(f"native status {status}")


count = 241
raw = (Sample * count)(*[Sample(i * .5, 8 * math.sin(i / 26) + 1.3 * math.sin(i * 1.8),
                               i * 8000, math.sin(math.pi * i / (count - 1)) ** 2, 1)
                          for i in range(count)])
options = Options(.35, .25, 1, 0)
handle = u64()
ok(lib.sm_create(c.byref(handle)))
try:
    ok(lib.sm_begin(handle, 5, 0x245d94ff))
    ok(lib.sm_append(handle, raw, count))
    state, written = State(), u32()
    ok(lib.sm_get_state(handle, c.byref(state)))
    derived = (Point * count)()
    ok(lib.sm_read_brush_points(handle, state.revision, 0, 0, c.byref(options), derived, count, c.byref(written)))
    assert written.value == count
    ok(lib.sm_finish(handle))
    required = u32()
    assert lib.sm_save(handle, None, 0, c.byref(required)) == 4
    snapshot = (c.c_ubyte * required.value)()
    ok(lib.sm_save(handle, snapshot, required, c.byref(required)))
    ok(lib.sm_load(handle, snapshot, required))
    ok(lib.sm_get_state(handle, c.byref(state)))
    restored = (Point * count)()
    ok(lib.sm_read_brush_points(handle, state.revision, 0, 0, c.byref(options), restored, count, c.byref(written)))
    assert bytes(derived) == bytes(restored)
finally:
    ok(lib.sm_destroy(handle))

with (args.output / "points.csv").open("w", newline="", encoding="utf-8") as file:
    writer = csv.writer(file)
    writer.writerow(["raw_x", "raw_y", "pressure", "smooth_x", "smooth_y", "width"])
    writer.writerows((r.x, r.y, r.pressure, p.x, p.y, p.width) for r, p in zip(raw, derived))


def roughness(values):
    return math.sqrt(sum((values[i+1] - 2*values[i] + values[i-1])**2
                         for i in range(1, len(values)-1)) / (len(values)-2))


raw_rms = roughness([p.y for p in raw])
derived_rms = roughness([p.y for p in derived])
metrics = {
    "dll_sha256": hashlib.sha256(args.dll.read_bytes()).hexdigest(),
    "samples": count, "alpha": options.alpha, "min_width_ratio": options.ratio,
    "raw_y_second_difference_rms": raw_rms, "smoothed_y_second_difference_rms": derived_rms,
    "roughness_ratio": derived_rms / raw_rms,
    "end_lag_page_units": math.hypot(raw[-1].x-derived[-1].x, raw[-1].y-derived[-1].y),
    "width_min": min(p.width for p in derived), "width_max": max(p.width for p in derived),
    "restored_geometry_byte_identical": True,
    "scope": "synthetic input; offline reference drawing; no device latency or feel measurement",
}
(args.output / "metrics.json").write_text(json.dumps(metrics, indent=2) + "\n", encoding="utf-8")

fig, axes = plt.subplots(3, 1, figsize=(12, 8.5), constrained_layout=True)
axes[0].plot([p.x for p in raw], [p.y for p in raw], color="#9aa3ae", lw=1, label="Raw (stored)")
axes[0].plot([p.x for p in derived], [p.y for p in derived], color="#245d94", lw=2, label="Derived")
axes[0].set_title("Causal smoothing | alpha = 0.35 | same 241 input samples", loc="left")
axes[0].legend(loc="upper right")

for a, b in zip(derived, list(derived)[1:]):
    steps = max(1, math.ceil(math.hypot(b.x-a.x, b.y-a.y) / .15))
    for i in range(steps):
        t = i / steps
        axes[1].add_patch(Circle((a.x + t*(b.x-a.x), a.y + t*(b.y-a.y)),
                                (a.width + t*(b.width-a.width))/2, color="#245d94", ec="none"))
axes[1].add_patch(Circle((derived[-1].x, derived[-1].y), derived[-1].width/2, color="#245d94", ec="none"))
axes[1].set_title("Reference pressure stroke | min ratio = 0.25 | discs in page coordinates", loc="left")
axes[1].set_aspect("equal", adjustable="box")

axes[2].plot([p.x for p in raw], [p.width for p in derived], color="#245d94", lw=2, label="Pressure width")
axes[2].axhline(5, color="#9aa3ae", ls="--", label="No pressure: fixed width 5")
axes[2].set_title("Width mapping | 1.25 to 5 page units", loc="left")
axes[2].set_ylim(0, 5.7)
axes[2].set_ylabel("Full width")
axes[2].legend(loc="lower right")
for ax in axes[:2]:
    ax.set_ylim(-12, 12)
    ax.set_ylabel("Page Y")
for ax in axes:
    ax.set_xlim(-3, 123)
    ax.set_xlabel("Page X")
    ax.grid(alpha=.15)
    ax.spines[["top", "right"]].set_visible(False)
fig.suptitle("Scratchmark M2 | actual DLL output, offline validation", fontsize=15)
fig.savefig(args.output / "brush-preview.png", dpi=150, metadata={"Software": "Scratchmark QA"})
plt.close(fig)
print(json.dumps(metrics, indent=2))
