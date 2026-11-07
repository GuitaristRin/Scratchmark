#include "scratchmark.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#if !defined(SM_STATIC) && defined(_WIN32)
#include <windows.h>
#elif !defined(SM_STATIC)
#include <dlfcn.h>
#endif

_Static_assert(sizeof(SmSample) == 32, "SmSample ABI");
_Static_assert(sizeof(SmState) == 32, "SmState ABI");
_Static_assert(sizeof(SmStrokeInfo) == 56, "SmStrokeInfo ABI");
_Static_assert(sizeof(SmBrushOptions) == 24, "SmBrushOptions ABI");
_Static_assert(sizeof(SmBrushPoint) == 24, "SmBrushPoint ABI");
_Static_assert(sizeof(SmViewport) == 32, "SmViewport ABI");
_Static_assert(sizeof(SmPoint) == 16, "SmPoint ABI");
_Static_assert(sizeof(SmBounds) == 32, "SmBounds ABI");

/* 两种链接路径共用行为断言，避免静态宿主只验证能链接。 */
#define API_LIST(X) \
    X(uint32_t, sm_abi_version, (void)) \
    X(uint64_t, sm_features, (void)) \
    X(SmStatus, sm_create, (SmHandle *)) \
    X(SmStatus, sm_destroy, (SmHandle)) \
    X(SmStatus, sm_begin, (SmHandle, double, uint32_t)) \
    X(SmStatus, sm_append, (SmHandle, const SmSample *, uint32_t)) \
    X(SmStatus, sm_finish, (SmHandle)) \
    X(SmStatus, sm_cancel, (SmHandle)) \
    X(SmStatus, sm_undo, (SmHandle)) \
    X(SmStatus, sm_redo, (SmHandle)) \
    X(SmStatus, sm_get_state, (SmHandle, SmState *)) \
    X(SmStatus, sm_get_stroke, (SmHandle, uint64_t, uint32_t, SmStrokeInfo *)) \
    X(SmStatus, sm_read_points, (SmHandle, uint64_t, uint32_t, uint32_t, SmSample *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_save, (SmHandle, uint8_t *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_load, (SmHandle, const uint8_t *, uint32_t)) \
    X(SmStatus, sm_read_brush_points, (SmHandle, uint64_t, uint32_t, uint32_t, const SmBrushOptions *, SmBrushPoint *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_get_page_id, (SmHandle, uint64_t *)) \
    X(SmStatus, sm_set_page_id, (SmHandle, uint64_t)) \
    X(SmStatus, sm_hit_test, (SmHandle, uint64_t, double, double, double, uint32_t *, uint32_t *)) \
    X(SmStatus, sm_erase_stroke, (SmHandle, uint64_t, uint32_t)) \
    X(SmStatus, sm_erase_path, (SmHandle, uint64_t, const SmPoint *, uint32_t, double)) \
    X(SmStatus, sm_lasso_hits, (SmHandle, uint64_t, const SmPoint *, uint32_t, uint32_t *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_erase_indices, (SmHandle, uint64_t, const uint32_t *, uint32_t)) \
    X(SmStatus, sm_clear, (SmHandle, uint64_t)) \
    X(SmStatus, sm_transform_strokes, (SmHandle, uint64_t, const uint32_t *, uint32_t, const SmTransform *)) \
    X(SmStatus, sm_copy_strokes, (SmHandle, uint64_t, const uint32_t *, uint32_t, uint8_t *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_paste, (SmHandle, uint64_t, const uint8_t *, uint32_t, const SmTransform *)) \
    X(SmStatus, sm_regularize_stroke, (SmHandle, uint64_t, uint32_t, uint32_t, uint32_t, SmShapeInfo *)) \
    X(SmStatus, sm_page_to_viewport_ex, (const SmViewportEx *, const SmPoint *, SmPoint *)) \
    X(SmStatus, sm_viewport_to_page_ex, (const SmViewportEx *, const SmPoint *, SmPoint *)) \
    X(SmStatus, sm_width_page_to_viewport_ex, (const SmViewportEx *, double, double *)) \
    X(SmStatus, sm_width_viewport_to_page_ex, (const SmViewportEx *, double, double *)) \
    X(SmStatus, sm_bounds_page_to_viewport_ex, (const SmViewportEx *, const SmBounds *, SmBounds *)) \
    X(SmStatus, sm_viewport_ex_zoom_at, (const SmViewportEx *, const SmPoint *, double, SmViewportEx *)) \
    X(SmStatus, sm_viewport_ex_fit, (const SmBounds *, double, double, double, double, SmViewportEx *)) \
    X(SmStatus, sm_predict, (SmHandle, uint64_t, const SmPredictionOptions *, SmBrushPoint *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_append_axes, (SmHandle, const SmSample *, const SmSampleAxes *, uint32_t)) \
    X(SmStatus, sm_read_axes, (SmHandle, uint64_t, uint32_t, uint32_t, SmSampleAxes *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_document_bounds, (SmHandle, uint64_t, SmBounds *, uint32_t *, uint32_t *)) \
    X(SmStatus, sm_translate_all, (SmHandle, uint64_t, double, double)) \
    X(SmStatus, sm_brush_preset, (uint32_t, uint32_t, double *)) \
    X(SmStatus, sm_brush_preset_width, (uint32_t, uint32_t, double, double *)) \
    X(SmStatus, sm_resample_stroke, (SmHandle, uint64_t, uint32_t, const SmResampleOptions *, SmBrushPoint *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_predict_ex, (SmHandle, uint64_t, const SmPredictionOptionsEx *, SmBrushPoint *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_cut_strokes, (SmHandle, uint64_t, const uint32_t *, uint32_t, uint8_t *, uint32_t, uint32_t *)) \
    X(SmStatus, sm_page_to_viewport, (const SmViewport *, const SmPoint *, SmPoint *)) \
    X(SmStatus, sm_viewport_to_page, (const SmViewport *, const SmPoint *, SmPoint *)) \
    X(SmStatus, sm_width_page_to_viewport, (const SmViewport *, double, double *)) \
    X(SmStatus, sm_width_viewport_to_page, (const SmViewport *, double, double *)) \
    X(SmStatus, sm_bounds_page_to_viewport, (const SmViewport *, const SmBounds *, SmBounds *))
#define FIELD(ret, name, args) ret (SM_CALL *name) args;
struct Api { API_LIST(FIELD) };
#undef FIELD
static struct Api api;
static unsigned checks;

#define REQUIRE(condition) do { \
    ++checks; \
    if (!(condition)) { fprintf(stderr, "check failed at line %d: %s\n", __LINE__, #condition); return 1; } \
} while (0)
#define STATUS(call, expected) do { \
    SmStatus actual = (call); \
    ++checks; \
    if (actual != (expected)) { fprintf(stderr, "status at line %d: %u != %u\n", \
        __LINE__, (unsigned)actual, (unsigned)(expected)); return 1; } \
} while (0)

#ifdef SM_STATIC
static int load_api(void) {
#define BIND(ret, name, args) api.name = name;
    API_LIST(BIND)
#undef BIND
    return 0;
}
#else
static int load_api(const char *path) {
#ifdef _WIN32
    HMODULE library = LoadLibraryExA(path, NULL,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
    if (!library) { fprintf(stderr, "library load failed: %lu\n", GetLastError()); return 1; }
#define SYMBOL(name) FARPROC symbol = GetProcAddress(library, #name)
#else
    void *library = dlopen(path, RTLD_NOW | RTLD_LOCAL);
    if (!library) { fprintf(stderr, "library load failed: %s\n", dlerror()); return 1; }
#define SYMBOL(name) void *symbol = dlsym(library, #name)
#endif
#define LOAD(ret, name, args) do { \
    SYMBOL(name); \
    if (!symbol) { fprintf(stderr, "missing export: %s\n", #name); return 1; } \
    _Static_assert(sizeof(api.name) == sizeof(symbol), "function pointer size"); \
    memcpy(&api.name, &symbol, sizeof(symbol)); \
} while (0);
    API_LIST(LOAD)
#undef LOAD
#undef SYMBOL
    /* 示例让库保持到进程结束，防止卸载时仍有函数指针被使用。 */
    return 0;
}
#endif

static int brush_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_BRUSH_GEOMETRY) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    STATUS(api.sm_begin(handle, 10.0, 0), SM_OK);
    SmSample points[3] = {{0, 0, 0, 0, 1}, {8, 4, 1, 0.5f, 1}, {8, 4, 2, 0, 0}};
    STATUS(api.sm_append(handle, points, 2), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    SmBrushOptions options = {0.5, 0.25, 1, 0};
    SmBrushPoint output[3];
    uint32_t written = 0;
    STATUS(api.sm_read_brush_points(handle, state.revision, 0, 0, &options, output, 3, &written), SM_OK);
    REQUIRE(written == 2 && output[0].x == 0 && output[0].width == 2.5);
    REQUIRE(output[1].x == 4 && output[1].y == 2 && output[1].width == 6.25);
    STATUS(api.sm_append(handle, points + 2, 1), SM_OK);
    written = 99; output[0].x = 99;
    STATUS(api.sm_read_brush_points(handle, state.revision, 0, 0, &options, output, 3, &written), SM_STALE_REVISION);
    REQUIRE(written == 99 && output[0].x == 99);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_read_brush_points(handle, state.revision, 0, 2, &options, output, 1, &written), SM_OK);
    REQUIRE(written == 1 && output[0].x == 6 && output[0].y == 3 && output[0].width == 10);
    options.alpha = 0;
    STATUS(api.sm_read_brush_points(handle, state.revision, 0, 0, &options, output, 3, &written), SM_INVALID_ARGUMENT);
    options.alpha = 1; options.pressure_enabled = 0;
    STATUS(api.sm_read_brush_points(handle, state.revision, 0, 1, &options, output, 1, &written), SM_OK);
    REQUIRE(output[0].x == 8 && output[0].y == 4 && output[0].width == 10);
    STATUS(api.sm_cancel(handle), SM_OK);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

static int viewport_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_PAGE_VIEWPORT) != 0);
    SmViewport vp = {2.0, 10.0, 20.0, 0};
    SmPoint page = {3.0, 4.0};
    SmPoint out = {0, 0};
    STATUS(api.sm_page_to_viewport(&vp, &page, &out), SM_OK);
    REQUIRE(out.x == 16.0 && out.y == 28.0);
    STATUS(api.sm_viewport_to_page(&vp, &out, &out), SM_OK);
    REQUIRE(out.x == 3.0 && out.y == 4.0);
    double width = 0;
    STATUS(api.sm_width_page_to_viewport(&vp, 5.0, &width), SM_OK);
    REQUIRE(width == 10.0);
    STATUS(api.sm_width_viewport_to_page(&vp, 10.0, &width), SM_OK);
    REQUIRE(width == 5.0);
    SmBounds bounds = {-1.0, -2.0, 3.0, 4.0};
    SmBounds out_bounds = {0, 0, 0, 0};
    STATUS(api.sm_bounds_page_to_viewport(&vp, &bounds, &out_bounds), SM_OK);
    REQUIRE(out_bounds.min_x == 8.0 && out_bounds.min_y == 16.0);
    REQUIRE(out_bounds.max_x == 16.0 && out_bounds.max_y == 28.0);
    /* 非法视口、越界页面域与溢出不写输出。 */
    SmViewport invalid = vp;
    invalid.scale = 0.0;
    SmPoint sentinel = {99.0, 98.0};
    SmPoint nan_point = {NAN, 0.0};
    SmPoint far_page = {2e9, 0.0};
    STATUS(api.sm_page_to_viewport(&invalid, &page, &sentinel), SM_INVALID_ARGUMENT);
    invalid = vp;
    invalid.reserved = 1;
    STATUS(api.sm_viewport_to_page(&invalid, &page, &sentinel), SM_INVALID_ARGUMENT);
    STATUS(api.sm_page_to_viewport(&vp, &nan_point, &sentinel), SM_INVALID_ARGUMENT);
    STATUS(api.sm_page_to_viewport(&vp, &far_page, &sentinel), SM_INVALID_ARGUMENT);
    REQUIRE(sentinel.x == 99.0 && sentinel.y == 98.0);
    /* 页面标识：空闲设置、同值不递增、活动笔划拒绝。 */
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    uint64_t page_id = 123;
    STATUS(api.sm_get_page_id(handle, &page_id), SM_OK);
    REQUIRE(page_id == 0);
    SmState state;
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_set_page_id(handle, 42), SM_INVALID_STATE);
    STATUS(api.sm_cancel(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    uint64_t before = state.revision;
    STATUS(api.sm_set_page_id(handle, 42), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.revision == before + 1);
    STATUS(api.sm_set_page_id(handle, 42), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.revision == before + 1);
    /* 设置页面标识后快照为格式 2；跨页标识冲突拒绝；无标识引擎可整份恢复。 */
    SmSample stroke[1] = {{1.0, 1.0, 0, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, stroke, 1), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    uint32_t required = 0;
    STATUS(api.sm_save(handle, NULL, 0, &required), SM_BUFFER_TOO_SMALL);
    uint8_t *bytes = malloc(required);
    REQUIRE(bytes != NULL);
    STATUS(api.sm_save(handle, bytes, required, &required), SM_OK);
    REQUIRE(memcmp(bytes, "SCRATCH2", 8) == 0);
    SmHandle other = 0;
    STATUS(api.sm_create(&other), SM_OK);
    STATUS(api.sm_set_page_id(other, 43), SM_OK);
    STATUS(api.sm_load(other, bytes, required), SM_INVALID_DATA);
    SmHandle fresh = 0;
    STATUS(api.sm_create(&fresh), SM_OK);
    STATUS(api.sm_load(fresh, bytes, required), SM_OK);
    page_id = 0;
    STATUS(api.sm_get_page_id(fresh, &page_id), SM_OK);
    REQUIRE(page_id == 42);
    free(bytes);
    STATUS(api.sm_destroy(handle), SM_OK);
    STATUS(api.sm_destroy(other), SM_OK);
    STATUS(api.sm_destroy(fresh), SM_OK);
    return 0;
}

static int hit_erase_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_HIT_ERASE) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample stroke[2] = {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 1, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, stroke, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    SmSample second[2] = {{100.0, 0.0, 0, 0.0f, 0}, {110.0, 0.0, 1, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, second, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    uint32_t index = 99, found = 99;
    STATUS(api.sm_hit_test(handle, state.revision, 5.0, 0.0, 0.0, &index, &found), SM_OK);
    REQUIRE(index == 0 && found == 1);
    index = 99; found = 99;
    STATUS(api.sm_hit_test(handle, state.revision, 1000.0, 0.0, 0.0, &index, &found), SM_OK);
    REQUIRE(index == 0 && found == 0);
    index = 99; found = 99;
    STATUS(api.sm_hit_test(handle, state.revision - 1, 5.0, 0.0, 0.0, &index, &found), SM_STALE_REVISION);
    REQUIRE(index == 99 && found == 99);
    STATUS(api.sm_hit_test(handle, state.revision, 5.0, 0.0, -1.0, &index, &found), SM_INVALID_ARGUMENT);
    REQUIRE(index == 99 && found == 99);
    STATUS(api.sm_hit_test(handle, state.revision, 5.0, 0.0, 0.0, NULL, &found), SM_INVALID_ARGUMENT);
    REQUIRE(found == 99);
    STATUS(api.sm_erase_stroke(handle, state.revision - 1, 0), SM_STALE_REVISION);
    STATUS(api.sm_erase_stroke(handle, state.revision, 2), SM_INVALID_ARGUMENT);
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    SmSample active = {0.0, 10.0, 0, 0.0f, 0};
    STATUS(api.sm_append(handle, &active, 1), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_erase_stroke(handle, state.revision, 0), SM_INVALID_STATE);
    index = 99; found = 99;
    STATUS(api.sm_hit_test(handle, state.revision, 0.0, 10.0, 0.0, &index, &found), SM_OK);
    REQUIRE(found == 0);
    STATUS(api.sm_cancel(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_erase_stroke(handle, state.revision, 0), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 1);
    STATUS(api.sm_hit_test(handle, state.revision, 5.0, 0.0, 0.0, &index, &found), SM_OK);
    REQUIRE(found == 0);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 2);
    STATUS(api.sm_hit_test(handle, state.revision, 5.0, 0.0, 0.0, &index, &found), SM_OK);
    REQUIRE(index == 0 && found == 1);
    STATUS(api.sm_redo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 1);
    STATUS(api.sm_hit_test(handle, state.revision, 5.0, 0.0, 0.0, &index, &found), SM_OK);
    REQUIRE(found == 0);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 局部擦除：路径切分、相切不切分、无效果不递增 revision、整笔覆盖与撤销。 */
static int partial_erase_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_PARTIAL_ERASE) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample stroke[2] = {{0.0, 0.0, 0, 0.0f, 1}, {10.0, 0.0, 1000, 1.0f, 1}};
    STATUS(api.sm_begin(handle, 2.0, 0x11223344), SM_OK);
    STATUS(api.sm_append(handle, stroke, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    SmPoint path[1] = {{5.0, 0.0}};
    STATUS(api.sm_erase_path(handle, state.revision - 1, path, 1, 2.0), SM_STALE_REVISION);
    STATUS(api.sm_erase_path(handle, state.revision, NULL, 1, 2.0), SM_INVALID_ARGUMENT);
    STATUS(api.sm_erase_path(handle, state.revision, path, 0, 2.0), SM_INVALID_ARGUMENT);
    STATUS(api.sm_erase_path(handle, state.revision, path, 1, 0.0), SM_INVALID_ARGUMENT);
    STATUS(api.sm_erase_path(handle, state.revision, path, 1, 1e9 + 1.0), SM_INVALID_ARGUMENT);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 1);
    STATUS(api.sm_erase_path(handle, state.revision, path, 1, 2.0), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 2 && state.can_undo == 1 && state.can_redo == 0);
    SmStrokeInfo info;
    STATUS(api.sm_get_stroke(handle, state.revision, 0, &info), SM_OK);
    REQUIRE(info.point_count == 2 && info.width == 2.0 && info.color == 0x11223344);
    REQUIRE(info.min_x == -1.0 && fabs(info.max_x - 4.0) < 1e-9);
    SmSample cut[2];
    uint32_t written = 0;
    STATUS(api.sm_read_points(handle, state.revision, 1, 0, cut, 2, &written), SM_OK);
    REQUIRE(written == 2 && fabs(cut[0].x - 7.0) < 1e-9 && cut[0].time_us == 700);
    /* M17 起切点同时置 bit1：压力位 + 切点位。 */
    REQUIRE((cut[0].flags & SM_FLAG_PRESSURE) != 0 && (cut[0].flags & SM_FLAG_CUT_END) != 0 && fabsf(cut[0].pressure - 0.7f) < 1e-6f);
    uint32_t index = 99, found = 99;
    STATUS(api.sm_hit_test(handle, state.revision, 5.0, 0.0, 0.0, &index, &found), SM_OK);
    REQUIRE(found == 0);
    /* 一次手势只占一个撤销条目：撤销恢复原笔划，重做再次切分。 */
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 1);
    uint64_t before = state.revision;
    SmPoint tangent[1] = {{5.0, 2.0}};
    STATUS(api.sm_erase_path(handle, state.revision, tangent, 1, 2.0), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.revision == before && state.stroke_count == 1);
    STATUS(api.sm_redo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 2);
    /* 整笔落在删除集合内则整笔消失。 */
    STATUS(api.sm_erase_path(handle, state.revision, path, 1, 100.0), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 0);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 2);
    /* 活动笔划期间拒绝。 */
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    SmSample dot = {0.0, 50.0, 0, 0.0f, 0};
    STATUS(api.sm_append(handle, &dot, 1), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_erase_path(handle, state.revision, path, 1, 2.0), SM_INVALID_STATE);
    STATUS(api.sm_cancel(handle), SM_OK);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 套索命中与批量删除：查询模式、容量不足、一次撤销、清空。 */
static int selection_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_SELECTION) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample lines[3][2] = {
        {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 1, 0.0f, 0}},
        {{100.0, 0.0, 0, 0.0f, 0}, {110.0, 0.0, 1, 0.0f, 0}},
        {{500.0, 0.0, 0, 0.0f, 0}, {510.0, 0.0, 1, 0.0f, 0}}
    };
    for (int i = 0; i < 3; i++) {
        STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
        STATUS(api.sm_append(handle, lines[i], 2), SM_OK);
        STATUS(api.sm_finish(handle), SM_OK);
    }
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    SmPoint lasso[4] = {{-20.0, -20.0}, {200.0, -20.0}, {200.0, 20.0}, {-20.0, 20.0}};

    /* 查询模式：不写数组，只回报所需数量。 */
    uint32_t written = 99, indices[8];
    for (int i = 0; i < 8; i++) indices[i] = 77;
    STATUS(api.sm_lasso_hits(handle, state.revision, lasso, 4, NULL, 0, &written), SM_BUFFER_TOO_SMALL);
    REQUIRE(written == 2);
    STATUS(api.sm_lasso_hits(handle, state.revision, lasso, 4, indices, 8, &written), SM_OK);
    REQUIRE(written == 2 && indices[0] == 0 && indices[1] == 1 && indices[2] == 77);
    STATUS(api.sm_lasso_hits(handle, state.revision, lasso, 4, indices, 1, &written), SM_BUFFER_TOO_SMALL);
    REQUIRE(written == 2 && indices[0] == 0);
    STATUS(api.sm_lasso_hits(handle, state.revision, lasso, 2, indices, 8, &written), SM_INVALID_ARGUMENT);
    STATUS(api.sm_lasso_hits(handle, state.revision - 1, lasso, 4, indices, 8, &written), SM_STALE_REVISION);

    /* 批量删除：一次调用一个撤销条目。 */
    uint32_t duplicate[2] = {0, 0};
    STATUS(api.sm_erase_indices(handle, state.revision, duplicate, 2), SM_INVALID_ARGUMENT);
    uint32_t selected[2] = {0, 1};
    STATUS(api.sm_erase_indices(handle, state.revision, selected, 2), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 1 && state.can_redo == 0);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 3);

    /* 活动笔划期间拒绝。 */
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    SmSample active = {0.0, 30.0, 0, 0.0f, 0};
    STATUS(api.sm_append(handle, &active, 1), SM_OK);
    STATUS(api.sm_erase_indices(handle, state.revision, selected, 1), SM_INVALID_STATE);
    STATUS(api.sm_clear(handle, state.revision), SM_INVALID_STATE);
    STATUS(api.sm_cancel(handle), SM_OK);

    /* 清空一个条目，一次撤销全部恢复；空文档清空不递增 revision。 */
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_clear(handle, state.revision), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 0);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 3);
    STATUS(api.sm_clear(handle, state.revision), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    uint64_t before = state.revision;
    STATUS(api.sm_clear(handle, state.revision), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.revision == before);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 选区变换与复制粘贴：平移、缩放、剪贴板字节与格式 2 拒绝。 */
static int transform_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_TRANSFORM) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample first[2] = {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 1, 0.0f, 0}};
    SmSample second[2] = {{100.0, 0.0, 0, 0.0f, 0}, {110.0, 0.0, 1, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0x11223344), SM_OK);
    STATUS(api.sm_append(handle, first, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, second, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);

    /* 平移：被选中的笔划移动，未选中的不动，一次撤销可回退。 */
    SmTransform shift = {1.0, 0.0, 0.0, 1.0, 10.0, 5.0};
    uint32_t index0 = 0;
    STATUS(api.sm_transform_strokes(handle, state.revision, &index0, 1, &shift), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    SmStrokeInfo info;
    STATUS(api.sm_get_stroke(handle, state.revision, 0, &info), SM_OK);
    REQUIRE(info.min_x == 9.0 && info.min_y == 4.0 && info.width == 2.0);
    STATUS(api.sm_get_stroke(handle, state.revision, 1, &info), SM_OK);
    REQUIRE(info.min_x == 99.0);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_get_stroke(handle, state.revision, 0, &info), SM_OK);
    REQUIRE(info.min_x == -1.0);

    /* 等比缩放 2×：det = 4，线宽 ×2。 */
    SmTransform twice = {2.0, 0.0, 0.0, 2.0, 0.0, 0.0};
    STATUS(api.sm_transform_strokes(handle, state.revision, &index0, 1, &twice), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_get_stroke(handle, state.revision, 0, &info), SM_OK);
    REQUIRE(info.min_x == -2.0 && info.width == 4.0);
    STATUS(api.sm_undo(handle), SM_OK);

    /* 退化矩阵与 NULL 都拒绝。 */
    SmTransform flat = {1.0, 0.0, 0.0, 0.0, 0.0, 0.0};
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_transform_strokes(handle, state.revision, &index0, 1, &flat), SM_INVALID_ARGUMENT);
    STATUS(api.sm_transform_strokes(handle, state.revision, &index0, 1, NULL), SM_INVALID_ARGUMENT);

    /* 复制：查询大小、取字节、粘贴回同一文档并置顶。 */
    uint32_t both[2] = {0, 1};
    uint32_t required = 0;
    STATUS(api.sm_copy_strokes(handle, state.revision, both, 2, NULL, 0, &required), SM_BUFFER_TOO_SMALL);
    REQUIRE(required > 8);
    uint8_t *clipboard = malloc(required);
    REQUIRE(clipboard != NULL);
    STATUS(api.sm_copy_strokes(handle, state.revision, both, 2, clipboard, required, &required), SM_OK);
    REQUIRE(memcmp(clipboard, "SCRATCH\0", 8) == 0);
    SmTransform drop = {1.0, 0.0, 0.0, 1.0, 0.0, 200.0};
    STATUS(api.sm_paste(handle, state.revision, clipboard, required, &drop), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 4);
    STATUS(api.sm_get_stroke(handle, state.revision, 2, &info), SM_OK);
    REQUIRE(info.min_y == 199.0);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 2);
    STATUS(api.sm_redo(handle), SM_OK);

    /* 格式 2（整份文档）不能当剪贴板内容；跨文档粘贴可以。 */
    STATUS(api.sm_set_page_id(handle, 7), SM_OK);
    uint32_t document_size = 0;
    STATUS(api.sm_save(handle, NULL, 0, &document_size), SM_BUFFER_TOO_SMALL);
    uint8_t *document_bytes = malloc(document_size);
    REQUIRE(document_bytes != NULL);
    STATUS(api.sm_save(handle, document_bytes, document_size, &document_size), SM_OK);
    REQUIRE(memcmp(document_bytes, "SCRATCH2", 8) == 0);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_paste(handle, state.revision, document_bytes, document_size, NULL), SM_INVALID_DATA);
    free(document_bytes);
    SmHandle other = 0;
    STATUS(api.sm_create(&other), SM_OK);
    STATUS(api.sm_get_state(other, &state), SM_OK);
    STATUS(api.sm_paste(other, state.revision, clipboard, required, NULL), SM_OK);
    STATUS(api.sm_get_state(other, &state), SM_OK);
    REQUIRE(state.stroke_count == 2);
    free(clipboard);
    STATUS(api.sm_destroy(other), SM_OK);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 形状规整：预览零副作用、直线 / 矩形 / 圆 / 箭头生成与退化拒绝。 */
static int shape_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_SHAPE) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample straight[3] = {{0.0, 0.0, 0, 0.0f, 0}, {5.0, 0.0, 500, 0.0f, 0}, {10.0, 0.0, 1000, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0x11223344), SM_OK);
    STATUS(api.sm_append(handle, straight, 3), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);

    /* 预览：只测量，不改文档。 */
    SmShapeInfo info;
    memset(&info, 0, sizeof(info));
    STATUS(api.sm_regularize_stroke(handle, state.revision, 0, SM_SHAPE_LINE, 1, &info), SM_OK);
    REQUIRE(info.kind == SM_SHAPE_LINE && info.stroke_count == 1 && info.point_count == 2);
    REQUIRE(info.error < 1e-9);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 1);

    /* 实际替换：一个撤销条目。 */
    STATUS(api.sm_regularize_stroke(handle, state.revision, 0, SM_SHAPE_LINE, 0, &info), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    SmStrokeInfo stroke;
    STATUS(api.sm_get_stroke(handle, state.revision, 0, &stroke), SM_OK);
    REQUIRE(stroke.point_count == 2 && stroke.width == 2.0 && stroke.color == 0x11223344);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_get_stroke(handle, state.revision, 0, &stroke), SM_OK);
    REQUIRE(stroke.point_count == 3);

    /* 箭头：3 笔 6 点。 */
    SmSample shaft[2] = {{0.0, 300.0, 0, 0.0f, 0}, {100.0, 300.0, 1000, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, shaft, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_regularize_stroke(handle, state.revision, 1, SM_SHAPE_ARROW, 0, &info), SM_OK);
    REQUIRE(info.stroke_count == 3 && info.point_count == 6);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    /* 原直笔划仍在 index 0，箭头展开成 3 笔：共 4 笔。 */
    REQUIRE(state.stroke_count == 4);

    /* 矩形 5 点闭合，圆 65 点闭合。 */
    SmSample box[4] = {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 1, 0.0f, 0}, {10.0, 10.0, 2, 0.0f, 0}, {0.0, 10.0, 3, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, box, 4), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    uint32_t box_index = state.stroke_count - 1;
    STATUS(api.sm_regularize_stroke(handle, state.revision, box_index, SM_SHAPE_RECTANGLE, 0, &info), SM_OK);
    REQUIRE(info.point_count == 5);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_regularize_stroke(handle, state.revision, box_index, SM_SHAPE_CIRCLE, 0, &info), SM_OK);
    REQUIRE(info.point_count == 65);

    /* 非法种类 / 非法 flags / 单点笔划。 */
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_regularize_stroke(handle, state.revision, 0, 99, 0, &info), SM_INVALID_ARGUMENT);
    STATUS(api.sm_regularize_stroke(handle, state.revision, 0, SM_SHAPE_LINE, 2, &info), SM_INVALID_ARGUMENT);
    SmSample dot = {5.0, 5.0, 0, 0.0f, 0};
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, &dot, 1), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_regularize_stroke(handle, state.revision, state.stroke_count - 1, SM_SHAPE_LINE, 0, &info), SM_INVALID_ARGUMENT);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 视口旋转：90° 映射、往返、外接框、锚点缩放、适应窗口与非法输入。 */
static int viewport_ex_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_VIEWPORT_ROTATION) != 0);
    SmViewportEx vp = {1.0, 1.5707963267948966, 0.0, 0.0, 0};
    SmPoint page = {10.0, 0.0};
    SmPoint view = {0.0, 0.0};
    STATUS(api.sm_page_to_viewport_ex(&vp, &page, &view), SM_OK);
    REQUIRE(fabs(view.x) < 1e-9 && fabs(view.y - 10.0) < 1e-9);
    SmPoint back = {0.0, 0.0};
    STATUS(api.sm_viewport_to_page_ex(&vp, &view, &back), SM_OK);
    REQUIRE(fabs(back.x - 10.0) < 1e-9 && fabs(back.y) < 1e-9);

    /* 外接框：90° 时宽高互换。 */
    SmBounds box = {0.0, 0.0, 100.0, 20.0};
    SmBounds rotated;
    STATUS(api.sm_bounds_page_to_viewport_ex(&vp, &box, &rotated), SM_OK);
    REQUIRE(fabs((rotated.max_x - rotated.min_x) - 20.0) < 1e-9);
    REQUIRE(fabs((rotated.max_y - rotated.min_y) - 100.0) < 1e-9);

    /* 线宽只随 scale。 */
    double width = 0.0;
    STATUS(api.sm_width_page_to_viewport_ex(&vp, 6.0, &width), SM_OK);
    REQUIRE(fabs(width - 6.0) < 1e-9);
    STATUS(api.sm_width_viewport_to_page_ex(&vp, 6.0, &width), SM_OK);
    REQUIRE(fabs(width - 6.0) < 1e-9);

    /* 以视口点为锚缩放：锚点下的页面点不动。 */
    SmViewportEx tilted = {1.3, 0.6, 12.0, -8.0, 0};
    SmPoint anchor = {240.0, 180.0};
    SmPoint before;
    STATUS(api.sm_viewport_to_page_ex(&tilted, &anchor, &before), SM_OK);
    SmViewportEx zoomed;
    STATUS(api.sm_viewport_ex_zoom_at(&tilted, &anchor, 1.8, &zoomed), SM_OK);
    SmPoint after;
    STATUS(api.sm_viewport_to_page_ex(&zoomed, &anchor, &after), SM_OK);
    REQUIRE(fabs(after.x - before.x) < 1e-9 && fabs(after.y - before.y) < 1e-9);
    REQUIRE(fabs(zoomed.scale - 2.34) < 1e-9);

    /* 适应窗口：90° 下页面中心落在视口中心。 */
    SmBounds page_box = {0.0, 0.0, 800.0, 600.0};
    SmViewportEx fitted;
    STATUS(api.sm_viewport_ex_fit(&page_box, 1000.0, 800.0, 1.5707963267948966, 20.0, &fitted), SM_OK);
    REQUIRE(fabs(fitted.scale - 760.0 / 800.0) < 1e-9);
    SmPoint center = {400.0, 300.0};
    SmPoint center_view;
    STATUS(api.sm_page_to_viewport_ex(&fitted, &center, &center_view), SM_OK);
    REQUIRE(fabs(center_view.x - 500.0) < 1e-9 && fabs(center_view.y - 400.0) < 1e-9);

    /* 非法输入不写输出。 */
    SmViewportEx bad = {0.0, 0.0, 0.0, 0.0, 0};
    SmPoint sentinel = {77.0, 88.0};
    STATUS(api.sm_page_to_viewport_ex(&bad, &page, &sentinel), SM_INVALID_ARGUMENT);
    REQUIRE(sentinel.x == 77.0 && sentinel.y == 88.0);
    SmViewportEx untouched = {9.0, 0.0, 0.0, 0.0, 0};
    STATUS(api.sm_viewport_ex_zoom_at(&tilted, &anchor, 0.0, &untouched), SM_INVALID_ARGUMENT);
    REQUIRE(untouched.scale == 9.0);
    STATUS(api.sm_viewport_ex_fit(&page_box, 100.0, 100.0, 0.0, 60.0, &untouched), SM_INVALID_ARGUMENT);
    return 0;
}

/* 预测笔迹：匀速外推、长度上限、空预测、非法选项与只读性。 */
static int predict_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_PREDICTION) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmPredictionOptions opts = {1000.0, 1000.0, 4, 0, {1.0, 1.0, 0, 0}};
    SmBrushPoint points[8];
    uint32_t written = 99;
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);

    /* 没有活动笔划：成功但零预测点。 */
    STATUS(api.sm_predict(handle, state.revision, &opts, points, 8, &written), SM_OK);
    REQUIRE(written == 0);

    /* 匀速活动笔划：0.1 单位/微秒。 */
    SmSample moving[3] = {{0.0, 0.0, 0, 0.0f, 0}, {50.0, 0.0, 500, 0.0f, 0}, {100.0, 0.0, 1000, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0x11223344), SM_OK);
    STATUS(api.sm_append(handle, moving, 3), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    written = 99;
    STATUS(api.sm_predict(handle, state.revision, &opts, points, 8, &written), SM_OK);
    REQUIRE(written == 4);
    REQUIRE(fabs(points[0].x - 125.0) < 1e-9 && fabs(points[3].x - 200.0) < 1e-9);
    REQUIRE(fabs(points[0].width - 2.0) < 1e-9);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.active_points == 3);

    /* 长度上限：总位移收缩到 10。 */
    SmPredictionOptions capped = {1000.0, 10.0, 5, 0, {1.0, 1.0, 0, 0}};
    STATUS(api.sm_predict(handle, state.revision, &capped, points, 8, &written), SM_OK);
    REQUIRE(written == 5 && fabs(points[4].x - 110.0) < 1e-9);

    /* 容量不足：回报所需数量，数组不动。 */
    points[0].x = 77.0;
    written = 99;
    STATUS(api.sm_predict(handle, state.revision, &opts, points, 1, &written), SM_BUFFER_TOO_SMALL);
    REQUIRE(written == 4 && points[0].x == 77.0);

    /* 非法选项与过期 revision。 */
    SmPredictionOptions zero_span = {0.0, 1000.0, 4, 0, {1.0, 1.0, 0, 0}};
    STATUS(api.sm_predict(handle, state.revision, &zero_span, points, 8, &written), SM_INVALID_ARGUMENT);
    SmPredictionOptions too_many = {1000.0, 1000.0, 65, 0, {1.0, 1.0, 0, 0}};
    STATUS(api.sm_predict(handle, state.revision, &too_many, points, 8, &written), SM_INVALID_ARGUMENT);
    STATUS(api.sm_predict(handle, state.revision - 1, &opts, points, 8, &written), SM_STALE_REVISION);

    /* 提交后预测为空，历史不受影响。 */
    STATUS(api.sm_finish(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_predict(handle, state.revision, &opts, points, 8, &written), SM_OK);
    REQUIRE(written == 0);
    REQUIRE(state.can_undo == 1);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 0);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 逐点轴向：往返、默认值、非法值拒绝、格式 3 快照。 */
static int axes_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_STYLUS_AXES) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample samples[2] = {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 1000, 0.0f, 0}};
    SmSampleAxes axes[2] = {{0.0f, 0.0f}, {0.75f, -2.0f}};
    STATUS(api.sm_begin(handle, 2.0, 0x11223344), SM_OK);
    STATUS(api.sm_append_axes(handle, samples, axes, 2), SM_OK);

    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    SmSampleAxes read[2];
    uint32_t written = 0;
    STATUS(api.sm_read_axes(handle, state.revision, 0, 0, read, 2, &written), SM_OK);
    REQUIRE(written == 2 && read[1].tilt == 0.75f && read[1].orientation == -2.0f);
    REQUIRE(fabsf(read[0].tilt) < 1e-6f);

    /* 非法轴向整批拒绝。 */
    SmSampleAxes bad[1] = {{2.0f, 0.0f}};
    STATUS(api.sm_append_axes(handle, samples, bad, 1), SM_INVALID_ARGUMENT);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.active_points == 2);

    /* 默认轴向：普通 append 写入 (0, 0)；时间必须非递减，接在 1000 之后。 */
    SmSample plain[1] = {{0.0, 20.0, 2000, 0.0f, 0}};
    STATUS(api.sm_append(handle, plain, 1), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    SmSampleAxes zero[1];
    STATUS(api.sm_read_axes(handle, state.revision, 0, 2, zero, 1, &written), SM_OK);
    REQUIRE(written == 1 && zero[0].tilt == 0.0f && zero[0].orientation == 0.0f);
    STATUS(api.sm_finish(handle), SM_OK);

    /* 带轴向的文档存成格式 3。 */
    uint32_t required = 0;
    STATUS(api.sm_save(handle, NULL, 0, &required), SM_BUFFER_TOO_SMALL);
    uint8_t *bytes = malloc(required);
    REQUIRE(bytes != NULL);
    STATUS(api.sm_save(handle, bytes, required, &required), SM_OK);
    REQUIRE(memcmp(bytes, "SCRATCH3", 8) == 0);
    SmHandle other = 0;
    STATUS(api.sm_create(&other), SM_OK);
    STATUS(api.sm_load(other, bytes, required), SM_OK);
    STATUS(api.sm_get_state(other, &state), SM_OK);
    STATUS(api.sm_read_axes(other, state.revision, 0, 0, read, 2, &written), SM_OK);
    REQUIRE(written == 2 && read[1].tilt == 0.75f && read[1].orientation == -2.0f);
    free(bytes);
    STATUS(api.sm_destroy(other), SM_OK);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 文档边界与整份平移：空文档、并集、一次撤销、越界与活动笔划。 */
static int extent_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_PAGE_EXTENT) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);

    /* 空文档：found=0 且边界全零。 */
    SmBounds bounds;
    uint32_t strokes = 99, found = 99;
    STATUS(api.sm_document_bounds(handle, state.revision, &bounds, &strokes, &found), SM_OK);
    REQUIRE(found == 0 && strokes == 0);
    REQUIRE(bounds.min_x == 0.0 && bounds.max_y == 0.0);

    SmSample first[2] = {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 10, 0.0f, 0}};
    SmSample second[2] = {{100.0, 50.0, 0, 0.0f, 0}, {120.0, 60.0, 10, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, first, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    STATUS(api.sm_begin(handle, 4.0, 0), SM_OK);
    STATUS(api.sm_append(handle, second, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);

    /* 并集含笔宽：第二笔宽 4，右侧多 2。 */
    STATUS(api.sm_document_bounds(handle, state.revision, &bounds, &strokes, &found), SM_OK);
    REQUIRE(found == 1 && strokes == 2);
    REQUIRE(bounds.min_x == -1.0 && bounds.min_y == -1.0);
    REQUIRE(bounds.max_x == 122.0 && bounds.max_y == 62.0);
    STATUS(api.sm_document_bounds(handle, state.revision - 1, &bounds, &strokes, &found), SM_STALE_REVISION);

    /* 整份平移：一次撤销整体回退。 */
    double before_min_y = bounds.min_y;
    STATUS(api.sm_translate_all(handle, state.revision, 0.0, 50.0), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_document_bounds(handle, state.revision, &bounds, &strokes, &found), SM_OK);
    REQUIRE(fabs(bounds.min_y - (before_min_y + 50.0)) < 1e-9);
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_document_bounds(handle, state.revision, &bounds, &strokes, &found), SM_OK);
    REQUIRE(fabs(bounds.min_y - before_min_y) < 1e-9);
    STATUS(api.sm_redo(handle), SM_OK);

    /* 越界与活动笔划拒绝。 */
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_translate_all(handle, state.revision, 1e9, 0.0), SM_INVALID_ARGUMENT);
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    SmSample active = {0.0, 0.0, 0, 0.0f, 0};
    STATUS(api.sm_append(handle, &active, 1), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_translate_all(handle, state.revision, 1.0, 1.0), SM_INVALID_STATE);
    STATUS(api.sm_cancel(handle), SM_OK);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 笔型尺寸预置：25 个组合、递增、宽度换算与非法输入。 */
static int preset_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_BRUSH_PRESET) != 0);
    for (uint32_t kind = SM_BRUSH_PENCIL; kind <= SM_BRUSH_ERASER; kind++) {
        double previous = 0.0;
        for (uint32_t level = 1; level <= SM_BRUSH_LEVELS; level++) {
            double ratio = 0.0;
            STATUS(api.sm_brush_preset(kind, level, &ratio), SM_OK);
            REQUIRE(ratio > 0.0 && ratio <= 1.0);
            REQUIRE(ratio > previous);
            previous = ratio;
        }
    }
    double ratio = 0.0, width = 0.0;
    STATUS(api.sm_brush_preset(SM_BRUSH_PENCIL, 3, &ratio), SM_OK);
    REQUIRE(fabs(ratio - 0.26) < 1e-12);
    STATUS(api.sm_brush_preset_width(SM_BRUSH_PENCIL, 3, 10.0, &width), SM_OK);
    REQUIRE(fabs(width - 2.6) < 1e-12);
    STATUS(api.sm_brush_preset_width(SM_BRUSH_MARKER, 5, 1e6, &width), SM_OK);
    REQUIRE(fabs(width - 750000.0) < 1e-9);

    /* 非法输入不写输出。 */
    ratio = 99.0; width = 99.0;
    STATUS(api.sm_brush_preset(0, 1, &ratio), SM_INVALID_ARGUMENT);
    STATUS(api.sm_brush_preset(SM_BRUSH_PENCIL, 6, &ratio), SM_INVALID_ARGUMENT);
    STATUS(api.sm_brush_preset_width(SM_BRUSH_PENCIL, 1, 0.0, &width), SM_INVALID_ARGUMENT);
    STATUS(api.sm_brush_preset_width(SM_BRUSH_PENCIL, 1, 1e6 + 1.0, &width), SM_INVALID_ARGUMENT);
    STATUS(api.sm_brush_preset(SM_BRUSH_PENCIL, 1, NULL), SM_INVALID_ARGUMENT);
    REQUIRE(ratio == 99.0 && width == 99.0);
    return 0;
}

/* 点间距重采样：数量、位置、末尾点、容量不足与非法参数。 */
static int resample_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_RESAMPLE) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample line[2] = {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 100, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0x11223344), SM_OK);
    STATUS(api.sm_append(handle, line, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);

    SmResampleOptions opts = {0.5, {1.0, 1.0, 0, 0}, 0};
    uint32_t written = 99;
    STATUS(api.sm_resample_stroke(handle, state.revision, 0, &opts, NULL, 0, &written), SM_BUFFER_TOO_SMALL);
    REQUIRE(written == 11);

    SmBrushPoint points[11];
    STATUS(api.sm_resample_stroke(handle, state.revision, 0, &opts, points, 11, &written), SM_OK);
    REQUIRE(written == 11);
    REQUIRE(fabs(points[0].x) < 1e-9 && fabs(points[5].x - 5.0) < 1e-9 && fabs(points[10].x - 10.0) < 1e-9);
    REQUIRE(fabs(points[10].width - 2.0) < 1e-9);

    /* 容量不足不写数组。 */
    points[0].x = 77.0;
    STATUS(api.sm_resample_stroke(handle, state.revision, 0, &opts, points, 3, &written), SM_BUFFER_TOO_SMALL);
    REQUIRE(written == 11 && points[0].x == 77.0);

    /* 非法参数。 */
    SmResampleOptions bad = {0.0, {1.0, 1.0, 0, 0}, 0};
    STATUS(api.sm_resample_stroke(handle, state.revision, 0, &bad, points, 11, &written), SM_INVALID_ARGUMENT);
    STATUS(api.sm_resample_stroke(handle, state.revision - 1, 0, &opts, points, 11, &written), SM_STALE_REVISION);
    STATUS(api.sm_resample_stroke(handle, state.revision, 9, &opts, points, 11, &written), SM_INVALID_ARGUMENT);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 预测刷新率：刷新率 0 与 M10 同参一致、240 Hz 压缩时间窗、非法值拒绝。 */
static int predict_ex_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_PREDICT_REFRESH) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample moving[3] = {{0.0, 0.0, 0, 0.0f, 0}, {50.0, 0.0, 500, 0.0f, 0}, {100.0, 0.0, 1000, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0x11223344), SM_OK);
    STATUS(api.sm_append(handle, moving, 3), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);

    SmPredictionOptions base = {1000.0, 10000.0, 4, 0, {1.0, 1.0, 0, 0}};
    SmPredictionOptionsEx ex_zero = {1000.0, 10000.0, 4, 0, {1.0, 1.0, 0, 0}, 0.0};
    SmBrushPoint legacy[4], extended[4];
    uint32_t written_a = 0, written_b = 0;
    STATUS(api.sm_predict(handle, state.revision, &base, legacy, 4, &written_a), SM_OK);
    STATUS(api.sm_predict_ex(handle, state.revision, &ex_zero, extended, 4, &written_b), SM_OK);
    REQUIRE(written_a == written_b && written_a == 4);
    for (uint32_t i = 0; i < 4; i++) {
        REQUIRE(fabs(legacy[i].x - extended[i].x) < 1e-9);
        REQUIRE(fabs(legacy[i].width - extended[i].width) < 1e-9);
    }

    /* 16 ms 窗与 240 Hz 一帧对比。 */
    SmPredictionOptionsEx ex_long = {16000.0, 10000.0, 4, 0, {1.0, 1.0, 0, 0}, 0.0};
    SmPredictionOptionsEx ex_frame = {16000.0, 10000.0, 4, 0, {1.0, 1.0, 0, 0}, 240.0};
    STATUS(api.sm_predict_ex(handle, state.revision, &ex_long, legacy, 4, &written_a), SM_OK);
    STATUS(api.sm_predict_ex(handle, state.revision, &ex_frame, extended, 4, &written_b), SM_OK);
    REQUIRE(fabs(legacy[3].x - 1700.0) < 1e-6);
    REQUIRE(fabs(extended[3].x - 516.6666667) < 1e-6);
    REQUIRE(extended[3].x < legacy[3].x);

    /* 非法刷新率。 */
    SmPredictionOptionsEx bad = {16000.0, 10000.0, 4, 0, {1.0, 1.0, 0, 0}, -1.0};
    STATUS(api.sm_predict_ex(handle, state.revision, &bad, extended, 4, &written_b), SM_INVALID_ARGUMENT);
    SmPredictionOptionsEx too_fast = {16000.0, 10000.0, 4, 0, {1.0, 1.0, 0, 0}, 10001.0};
    STATUS(api.sm_predict_ex(handle, state.revision, &too_fast, extended, 4, &written_b), SM_INVALID_ARGUMENT);
    STATUS(api.sm_predict_ex(handle, state.revision - 1, &ex_frame, extended, 4, &written_b), SM_STALE_REVISION);
    STATUS(api.sm_cancel(handle), SM_OK);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 原子剪切：查询不剪切、剪贴板可粘贴、一次撤销整体恢复。 */
static int cut_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_ATOMIC_CUT) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample first[2] = {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 10, 0.0f, 0}};
    SmSample second[2] = {{0.0, 100.0, 0, 0.0f, 0}, {10.0, 100.0, 10, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, first, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    STATUS(api.sm_append(handle, second, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);

    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    uint32_t target = 0;
    uint32_t required = 0;
    STATUS(api.sm_cut_strokes(handle, state.revision, &target, 1, NULL, 0, &required), SM_BUFFER_TOO_SMALL);
    REQUIRE(required > 8);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 2); /* 查询模式没有剪切 */

    uint8_t *bytes = malloc(required);
    REQUIRE(bytes != NULL);
    uint32_t written = 0;
    STATUS(api.sm_cut_strokes(handle, state.revision, &target, 1, bytes, required, &written), SM_OK);
    REQUIRE(written == required);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 1);

    /* 一次撤销整体恢复。 */
    STATUS(api.sm_undo(handle), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 2);

    /* 剪贴板内容能粘贴回原几何。 */
    STATUS(api.sm_paste(handle, state.revision, bytes, written, NULL), SM_OK);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 3);
    SmStrokeInfo info;
    STATUS(api.sm_get_stroke(handle, state.revision, 2, &info), SM_OK);
    REQUIRE(fabs(info.min_y - (-1.0)) < 1e-9);

    /* 非法输入不改文档。 */
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    STATUS(api.sm_cut_strokes(handle, state.revision, &target, 0, bytes, required, &written), SM_INVALID_ARGUMENT);
    uint32_t out_of_range = 99;
    STATUS(api.sm_cut_strokes(handle, state.revision, &out_of_range, 1, bytes, required, &written), SM_INVALID_ARGUMENT);
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 3);
    free(bytes);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

/* 擦除切点标记：切出来的端点带 bit1，原始端点不带，快照往返保留。 */
static int cut_mark_smoke(void) {
    REQUIRE((api.sm_features() & SM_FEATURE_CUT_MARK) != 0);
    SmHandle handle = 0;
    STATUS(api.sm_create(&handle), SM_OK);
    SmSample line[2] = {{0.0, 0.0, 0, 0.0f, 0}, {10.0, 0.0, 100, 0.0f, 0}};
    STATUS(api.sm_begin(handle, 2.0, 0x11223344), SM_OK);
    STATUS(api.sm_append(handle, line, 2), SM_OK);
    STATUS(api.sm_finish(handle), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(handle, &state), SM_OK);
    SmPoint path = {5.0, 0.0};
    STATUS(api.sm_erase_path(handle, state.revision, &path, 1, 2.0), SM_OK);

    STATUS(api.sm_get_state(handle, &state), SM_OK);
    REQUIRE(state.stroke_count == 2);
    SmSample read[4];
    uint32_t written = 0;
    SmStrokeInfo info;
    STATUS(api.sm_get_stroke(handle, state.revision, 0, &info), SM_OK);
    STATUS(api.sm_read_points(handle, state.revision, 0, 0, read, info.point_count, &written), SM_OK);
    REQUIRE((read[0].flags & SM_FLAG_CUT_END) == 0);
    REQUIRE((read[written - 1].flags & SM_FLAG_CUT_END) != 0);
    STATUS(api.sm_get_stroke(handle, state.revision, 1, &info), SM_OK);
    STATUS(api.sm_read_points(handle, state.revision, 1, 0, read, info.point_count, &written), SM_OK);
    REQUIRE((read[0].flags & SM_FLAG_CUT_END) != 0);
    REQUIRE((read[written - 1].flags & SM_FLAG_CUT_END) == 0);

    /* 标记不需要新快照格式。 */
    uint32_t required = 0;
    STATUS(api.sm_save(handle, NULL, 0, &required), SM_BUFFER_TOO_SMALL);
    uint8_t *bytes = malloc(required);
    REQUIRE(bytes != NULL);
    STATUS(api.sm_save(handle, bytes, required, &required), SM_OK);
    REQUIRE(memcmp(bytes, "SCRATCH\0", 8) == 0);
    free(bytes);

    /* 写入通道拒绝伪造的切点位。 */
    STATUS(api.sm_begin(handle, 2.0, 0), SM_OK);
    SmSample forged = {0.0, 0.0, 0, 0.0f, SM_FLAG_CUT_END};
    STATUS(api.sm_append(handle, &forged, 1), SM_INVALID_ARGUMENT);
    STATUS(api.sm_cancel(handle), SM_OK);
    STATUS(api.sm_destroy(handle), SM_OK);
    return 0;
}

int main(int argc, char **argv) {
#ifdef SM_STATIC
    if (argc != 2) { fprintf(stderr, "usage: smoke SNAPSHOT_PATH\n"); return 2; }
    const char *snapshot_path = argv[1];
    if (load_api()) { return 1; }
#else
    if (argc != 3) { fprintf(stderr, "usage: smoke ABS_LIBRARY SNAPSHOT_PATH\n"); return 2; }
    const char *snapshot_path = argv[2];
    if (load_api(argv[1])) { return 1; }
#endif
    REQUIRE(api.sm_abi_version() == 1);
    STATUS(api.sm_create(NULL), SM_INVALID_ARGUMENT);
    SmHandle first = 0;
    STATUS(api.sm_create(&first), SM_OK);
    REQUIRE(first != 0);
    STATUS(api.sm_begin(first, 2.0, 0x11223344), SM_OK);
    SmSample samples[2] = {{0.0, 1.0, 10, 0.0f, 0}, {2.0, -1.0, 20, 0.5f, 1}};
    STATUS(api.sm_append(first, samples, 2), SM_OK);
    SmState state;
    STATUS(api.sm_get_state(first, &state), SM_OK);
    REQUIRE(state.has_active == 1 && state.active_points == 2 && state.stroke_count == 0);
    SmStrokeInfo info;
    STATUS(api.sm_get_stroke(first, state.revision, 0, &info), SM_OK);
    REQUIRE(info.min_x == -1.0 && info.min_y == -2.0 && info.max_x == 3.0 && info.max_y == 2.0);
    SmSample invalid = {NAN, 0.0, 30, 0.0f, 0};
    STATUS(api.sm_append(first, &invalid, 1), SM_INVALID_ARGUMENT);
    STATUS(api.sm_get_stroke(first, state.revision, 0, &info), SM_OK);
    REQUIRE(info.point_count == 2);
    uint32_t required = 0;
    STATUS(api.sm_save(first, NULL, 0, &required), SM_INVALID_STATE);
    STATUS(api.sm_finish(first), SM_OK);
    STATUS(api.sm_get_stroke(first, state.revision, 0, &info), SM_STALE_REVISION);
    STATUS(api.sm_save(first, NULL, 0, &required), SM_BUFFER_TOO_SMALL);
    REQUIRE(required == 96);
    uint8_t *bytes = malloc(required);
    REQUIRE(bytes != NULL);
    memset(bytes, 0xaa, required);
    STATUS(api.sm_save(first, bytes, required - 1, &required), SM_BUFFER_TOO_SMALL);
    REQUIRE(bytes[0] == 0xaa && bytes[95] == 0xaa);
    STATUS(api.sm_save(first, bytes, required, &required), SM_OK);
    FILE *file = fopen(snapshot_path, "wb");
    REQUIRE(file != NULL);
    REQUIRE(fwrite(bytes, 1, required, file) == required);
    REQUIRE(fclose(file) == 0);
    STATUS(api.sm_destroy(first), SM_OK);
    STATUS(api.sm_get_state(first, &state), SM_INVALID_HANDLE);
    memset(bytes, 0, required);
    file = fopen(snapshot_path, "rb");
    REQUIRE(file != NULL);
    REQUIRE(fread(bytes, 1, required, file) == required);
    REQUIRE(fgetc(file) == EOF);
    REQUIRE(fclose(file) == 0);
    SmHandle second = 0;
    STATUS(api.sm_create(&second), SM_OK);
    REQUIRE(second != first);
    STATUS(api.sm_load(second, bytes, required), SM_OK);
    STATUS(api.sm_get_state(second, &state), SM_OK);
    REQUIRE(state.stroke_count == 1 && state.has_active == 0 && state.can_redo == 0);
    STATUS(api.sm_get_stroke(second, state.revision, 0, &info), SM_OK);
    REQUIRE(info.width == 2.0 && info.color == 0x11223344 && info.point_count == 2);
    SmSample output[2];
    uint32_t written = 99;
    STATUS(api.sm_read_points(second, state.revision, 0, 0, output, 2, &written), SM_OK);
    REQUIRE(written == 2 && memcmp(output, samples, sizeof(samples)) == 0);
    bytes[0] ^= 1;
    STATUS(api.sm_load(second, bytes, required), SM_INVALID_DATA);
    STATUS(api.sm_get_stroke(second, state.revision, 0, &info), SM_OK);
    free(bytes);
    STATUS(api.sm_undo(second), SM_OK);
    STATUS(api.sm_begin(second, 1.0, 0xff0000ff), SM_OK);
    STATUS(api.sm_append(second, samples, 1), SM_OK);
    STATUS(api.sm_cancel(second), SM_OK);
    STATUS(api.sm_redo(second), SM_OK);
    STATUS(api.sm_begin(second, 3.0, 0x00ff00ff), SM_OK);
    STATUS(api.sm_append(second, samples, 2), SM_OK);
    STATUS(api.sm_finish(second), SM_OK);
    STATUS(api.sm_get_state(second, &state), SM_OK);
    REQUIRE(state.stroke_count == 2);
    STATUS(api.sm_destroy(second), SM_OK);
    STATUS(api.sm_destroy(second), SM_INVALID_HANDLE);
    if (brush_smoke()) { return 1; }
    if (viewport_smoke()) { return 1; }
    if (hit_erase_smoke()) { return 1; }
    if (cut_mark_smoke()) { return 1; }
    if (cut_smoke()) { return 1; }
    if (predict_ex_smoke()) { return 1; }
    if (resample_smoke()) { return 1; }
    if (preset_smoke()) { return 1; }
    if (extent_smoke()) { return 1; }
    if (axes_smoke()) { return 1; }
    if (predict_smoke()) { return 1; }
    if (viewport_ex_smoke()) { return 1; }
    if (shape_smoke()) { return 1; }
    if (transform_smoke()) { return 1; }
    if (selection_smoke()) { return 1; }
    if (partial_erase_smoke()) { return 1; }
    printf("native smoke: %u checks passed\n", checks);
    return 0;
}
