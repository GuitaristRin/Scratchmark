#include "scratchmark.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#endif

_Static_assert(sizeof(SmSample) == 32, "SmSample ABI");
_Static_assert(sizeof(SmState) == 32, "SmState ABI");
_Static_assert(sizeof(SmStrokeInfo) == 56, "SmStrokeInfo ABI");

/* 只通过运行时符号加载，不链接引擎源码或导入库。 */
#define API_LIST(X) \
    X(uint32_t, sm_abi_version, (void)) \
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
    X(SmStatus, sm_load, (SmHandle, const uint8_t *, uint32_t))
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

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: smoke ABS_LIBRARY SNAPSHOT_PATH\n"); return 2; }
    if (load_api(argv[1])) { return 1; }
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
    FILE *file = fopen(argv[2], "wb");
    REQUIRE(file != NULL);
    REQUIRE(fwrite(bytes, 1, required, file) == required);
    REQUIRE(fclose(file) == 0);
    STATUS(api.sm_destroy(first), SM_OK);
    STATUS(api.sm_get_state(first, &state), SM_INVALID_HANDLE);
    memset(bytes, 0, required);
    file = fopen(argv[2], "rb");
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
    printf("native smoke: %u checks passed\n", checks);
    return 0;
}
