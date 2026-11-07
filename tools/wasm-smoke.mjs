/**
 * WebAssembly 冒烟：在 Node 里直接调用 C ABI，验证的是“同一份引擎代码换平台后行为不变”。
 *
 * 与 examples/c/smoke.c 的关系：不是逐条复刻 662 项，而是覆盖编辑闭环的关键路径
 * （写入 / 派生 / 快照往返 / 撤销重做 / 擦除切分与切点标记 / 整份平移 / 边界 / 预置 / 重采样）。
 * 用法：node tools/wasm-smoke.mjs <path-to-scratchmark.wasm>
 */
import { readFile } from "node:fs/promises";

const path = process.argv[2];
if (!path) {
  console.error("usage: node tools/wasm-smoke.mjs <scratchmark.wasm>");
  process.exit(2);
}

const bytes = await readFile(path);
const { instance } = await WebAssembly.instantiate(bytes, {});
const api = instance.exports;
const memory = api.memory;

let checks = 0;
function require_(condition, what) {
  checks++;
  if (!condition) {
    console.error(`check failed: ${what}`);
    process.exit(1);
  }
}
function status(call, expected, what) {
  const actual = call();
  require_(actual === expected, `${what} (status ${actual} != ${expected})`);
}

// 把 JS 数组写进 wasm 线性内存，返回指针。
function writeBytes(size, fill) {
  const pointer = api.sm_alloc(size);
  if (!pointer) throw new Error(`sm_alloc(${size}) failed`);
  fill(new DataView(memory.buffer), pointer);
  return pointer;
}

const SM_OK = 0;
const SM_BUFFER_TOO_SMALL = 4;
const SAMPLE_SIZE = 32; // x f64, y f64, time u64, pressure f32, flags u32

function writeSamples(points) {
  const size = points.length * SAMPLE_SIZE;
  return writeBytes(size, (view, base) => {
    points.forEach((point, index) => {
      const at = base + index * SAMPLE_SIZE;
      view.setFloat64(at, point.x, true);
      view.setFloat64(at + 8, point.y, true);
      view.setBigUint64(at + 16, BigInt(point.time), true);
      view.setFloat32(at + 24, point.pressure ?? 0, true);
      view.setUint32(at + 28, point.flags ?? 0, true);
    });
  });
}

// 1. 版本与能力位
require_(api.sm_abi_version() === 1, "abi version is 1");
const features = api.sm_features();
require_(Number(features & 65535n) === 65535, "features cover bits 0-15");
require_((features & (1n << 16n)) !== 0n, "features advertise host alloc");

// 2. 建文档、写一笔
const handlePtr = api.sm_alloc(8);
require_(handlePtr !== 0, "handle slot allocated");
status(() => api.sm_create(handlePtr), SM_OK, "create");
const view = new DataView(memory.buffer);
const handle = view.getBigUint64(handlePtr, true);
require_(handle !== 0, "handle is non-zero");

status(() => api.sm_begin(handle, 3.0, 0x11223344), SM_OK, "begin");
const samples = [];
for (let i = 0; i <= 40; i++) {
  samples.push({ x: i * 5, y: (i % 10) * 2, time: i * 1000 });
}
const samplePtr = writeSamples(samples);
status(() => api.sm_append(handle, samplePtr, samples.length), SM_OK, "append");
status(() => api.sm_finish(handle), SM_OK, "finish");

const statePtr = api.sm_alloc(40);
status(() => api.sm_get_state(handle, statePtr), SM_OK, "get state");
const state = readState(statePtr, new DataView(memory.buffer));
require_(state.strokeCount === 1, "one committed stroke");
require_(state.revision >= 2n, `revision after begin/finish (${state.revision})`);

// 3. 派生笔刷点（读取类接口是截断语义：先问点数再取）
const requiredPtr = api.sm_alloc(4);
const infoPtrEarly = api.sm_alloc(56);
status(() => api.sm_get_stroke(handle, state.revision, 0, infoPtrEarly), SM_OK, "stroke info");
const earlyInfo = readStrokeInfo(infoPtrEarly, new DataView(memory.buffer));
require_(earlyInfo.pointCount === samples.length, `point count ${earlyInfo.pointCount}`);
const brushOptions = brushOptionsPtr();
const brushBuf = api.sm_alloc(earlyInfo.pointCount * 24);
status(
  () => api.sm_read_brush_points(handle, state.revision, 0, 0, brushOptions, brushBuf,
    earlyInfo.pointCount, requiredPtr),
  SM_OK,
  "read brush points",
);
const brushWritten = new DataView(memory.buffer).getUint32(requiredPtr, true);
require_(brushWritten === samples.length, `brush point count ${brushWritten}`);

// 4. 快照往返
let snapshotSize = 0;
status(() => api.sm_save(handle, 0, 0, requiredPtr), SM_BUFFER_TOO_SMALL, "save probe");
snapshotSize = new DataView(memory.buffer).getUint32(requiredPtr, true);
require_(snapshotSize > 8, `snapshot size ${snapshotSize}`);
const snapshotPtr = api.sm_alloc(snapshotSize);
status(() => api.sm_save(handle, snapshotPtr, snapshotSize, requiredPtr), SM_OK, "save");
const magic = String.fromCharCode(...new Uint8Array(memory.buffer, snapshotPtr, 8));
require_(magic.startsWith("SCRATCH"), `snapshot magic ${JSON.stringify(magic)}`);

const otherPtr = api.sm_alloc(8);
status(() => api.sm_create(otherPtr), SM_OK, "create second");
const other = new DataView(memory.buffer).getBigUint64(otherPtr, true);
status(() => api.sm_load(other, snapshotPtr, snapshotSize), SM_OK, "load");
status(() => api.sm_get_state(other, statePtr), SM_OK, "state after load");
require_(readState(statePtr, new DataView(memory.buffer)).strokeCount === 1, "load keeps the stroke");

// 5. 撤销 / 重做
status(() => api.sm_undo(other), SM_OK, "undo");
status(() => api.sm_get_state(other, statePtr), SM_OK, "state after undo");
require_(readState(statePtr, new DataView(memory.buffer)).strokeCount === 0, "undo removes the stroke");
status(() => api.sm_redo(other), SM_OK, "redo");
status(() => api.sm_get_state(other, statePtr), SM_OK, "state after redo");
require_(readState(statePtr, new DataView(memory.buffer)).strokeCount === 1, "redo restores it");

// 6. 局部擦除：切成两段，切口带 bit1
status(() => api.sm_get_state(other, statePtr), SM_OK, "state before erase");
let revision = readState(statePtr, new DataView(memory.buffer)).revision;
const pathPtr = writeBytes(16, (view, base) => {
  view.setFloat64(base, 100, true);
  view.setFloat64(base + 8, 10, true);
});
status(() => api.sm_erase_path(other, revision, pathPtr, 1, 12.0), SM_OK, "erase path");
status(() => api.sm_get_state(other, statePtr), SM_OK, "state after erase");
const split = readState(statePtr, new DataView(memory.buffer));
require_(split.strokeCount === 2, `erase splits into 2 fragments (got ${split.strokeCount})`);

const infoPtr = api.sm_alloc(56);
const pointBuf = api.sm_alloc(64 * SAMPLE_SIZE);
status(() => api.sm_get_stroke(other, split.revision, 0, infoPtr), SM_OK, "stroke info");
const info = readStrokeInfo(infoPtr, new DataView(memory.buffer));
require_(info.pointCount > 2, `fragment keeps interior vertices (${info.pointCount} points)`);
status(
  () => api.sm_read_points(other, split.revision, 0, 0, pointBuf, info.pointCount, requiredPtr),
  SM_OK,
  "read points",
);
const written = new DataView(memory.buffer).getUint32(requiredPtr, true);
require_(written === info.pointCount, "read point count matches");
const lastFlags = new DataView(memory.buffer).getUint32(pointBuf + (written - 1) * SAMPLE_SIZE + 28, true);
require_((lastFlags & 2) === 2, `cut end is marked (flags ${lastFlags})`);
const firstFlags = new DataView(memory.buffer).getUint32(pointBuf + 28, true);
require_((firstFlags & 2) === 0, "original start is not marked");

// 7. 文档边界与整份平移
const boundsPtr = api.sm_alloc(32);
const strokeCountPtr = api.sm_alloc(4);
const foundPtr = api.sm_alloc(4);
status(() => api.sm_get_state(other, statePtr), SM_OK, "state before bounds");
revision = readState(statePtr, new DataView(memory.buffer)).revision;
status(
  () => api.sm_document_bounds(other, revision, boundsPtr, strokeCountPtr, foundPtr),
  SM_OK,
  "document bounds",
);
const before = readBounds(boundsPtr, new DataView(memory.buffer));
require_(new DataView(memory.buffer).getUint32(foundPtr, true) === 1, "bounds found");
status(() => api.sm_translate_all(other, revision, 0, 25), SM_OK, "translate all");
status(() => api.sm_get_state(other, statePtr), SM_OK, "state after translate");
revision = readState(statePtr, new DataView(memory.buffer)).revision;
status(
  () => api.sm_document_bounds(other, revision, boundsPtr, strokeCountPtr, foundPtr),
  SM_OK,
  "bounds after translate",
);
const after = readBounds(boundsPtr, new DataView(memory.buffer));
require_(Math.abs(after.minY - (before.minY + 25)) < 1e-9, "translate moved the content");

// 8. 笔型预置与重采样
const ratioPtr = api.sm_alloc(8);
status(() => api.sm_brush_preset(1, 3, ratioPtr), SM_OK, "brush preset");
require_(Math.abs(new DataView(memory.buffer).getFloat64(ratioPtr, true) - 0.26) < 1e-12, "pencil level 3 ratio");

const resamplePtr = api.sm_alloc(40);
const resampleOptsPtr = api.sm_alloc(32);
writeResampleOptions(resampleOptsPtr, new DataView(memory.buffer));
status(
  () => api.sm_resample_stroke(other, revision, 0, resampleOptsPtr, 0, 0, requiredPtr),
  SM_BUFFER_TOO_SMALL,
  "resample probe",
);
const resampled = new DataView(memory.buffer).getUint32(requiredPtr, true);
require_(resampled > 2, `resample produces points (${resampled})`);

// 9. 释放
status(() => api.sm_destroy(other), SM_OK, "destroy second");
status(() => api.sm_destroy(handle), SM_OK, "destroy first");

console.log(`wasm smoke: ${checks} checks passed`);

function readState(pointer, view) {
  return {
    revision: view.getBigUint64(pointer, true),
    strokeCount: view.getUint32(pointer + 8, true),
    activePoints: view.getUint32(pointer + 12, true),
  };
}

function readStrokeInfo(pointer, view) {
  return {
    width: view.getFloat64(pointer, true),
    minX: view.getFloat64(pointer + 8, true),
    minY: view.getFloat64(pointer + 16, true),
    maxX: view.getFloat64(pointer + 24, true),
    maxY: view.getFloat64(pointer + 32, true),
    color: view.getUint32(pointer + 40, true),
    pointCount: view.getUint32(pointer + 44, true),
  };
}

function readBounds(pointer, view) {
  return {
    minX: view.getFloat64(pointer, true),
    minY: view.getFloat64(pointer + 8, true),
    maxX: view.getFloat64(pointer + 16, true),
    maxY: view.getFloat64(pointer + 24, true),
  };
}

function brushOptionsPtr() {
  const pointer = api.sm_alloc(24);
  const view = new DataView(memory.buffer);
  view.setFloat64(pointer, 0.5, true);
  view.setFloat64(pointer + 8, 0.25, true);
  view.setUint32(pointer + 16, 1, true);
  view.setUint32(pointer + 20, 0, true);
  return pointer;
}

function writeResampleOptions(pointer, view) {
  view.setFloat64(pointer, 0.3, true); // spacing_ratio
  view.setFloat64(pointer + 8, 0.5, true); // brush.alpha
  view.setFloat64(pointer + 16, 0.25, true); // brush.min_width_ratio
  view.setUint32(pointer + 24, 1, true); // brush.pressure_enabled
  view.setUint32(pointer + 28, 0, true); // brush.reserved
}
