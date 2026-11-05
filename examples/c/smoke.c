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
    printf("native smoke: %u checks passed\n", checks);
    return 0;
}
