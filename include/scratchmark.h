#ifndef SCRATCHMARK_H
#define SCRATCHMARK_H
#include <stdint.h>

#if defined(_WIN32)
#define SM_CALL __cdecl
#else
#define SM_CALL
#endif
#ifdef __cplusplus
extern "C" {
#endif

/* 实验 ABI；缓冲区、状态和快照语义参 spec/M0.md。 */
typedef uint64_t SmHandle;
typedef uint32_t SmStatus;
enum {
    SM_OK = 0, SM_INVALID_ARGUMENT = 1, SM_INVALID_HANDLE = 2,
    SM_INVALID_STATE = 3, SM_BUFFER_TOO_SMALL = 4, SM_STALE_REVISION = 5,
    SM_INVALID_DATA = 6, SM_UNSUPPORTED_VERSION = 7, SM_LIMIT_EXCEEDED = 8,
    SM_INTERNAL = 9
};
typedef struct SmSample {
    double x, y;
    uint64_t time_us;
    float pressure;
    uint32_t flags;
} SmSample;
typedef struct SmState {
    uint64_t revision;
    uint32_t stroke_count, active_points, has_active, can_undo, can_redo;
    uint32_t reserved;
} SmState;
typedef struct SmStrokeInfo {
    double width, min_x, min_y, max_x, max_y;
    uint32_t color, point_count, active, reserved;
} SmStrokeInfo;

/* M2 派生几何；参数、采样滞后与宿主配置保存责任参 spec/M2.md。 */
typedef struct SmBrushOptions {
    double alpha, min_width_ratio;
    uint32_t pressure_enabled, reserved;
} SmBrushOptions;
typedef struct SmBrushPoint {
    double x, y, width;
} SmBrushPoint;
#define SM_FEATURE_BRUSH_GEOMETRY UINT64_C(1)

uint32_t SM_CALL sm_abi_version(void);
uint64_t SM_CALL sm_features(void);
SmStatus SM_CALL sm_create(SmHandle *out_handle);
SmStatus SM_CALL sm_destroy(SmHandle handle);
SmStatus SM_CALL sm_begin(SmHandle handle, double width, uint32_t color);
SmStatus SM_CALL sm_append(SmHandle handle, const SmSample *samples, uint32_t count);
SmStatus SM_CALL sm_finish(SmHandle handle);
SmStatus SM_CALL sm_cancel(SmHandle handle);
SmStatus SM_CALL sm_undo(SmHandle handle);
SmStatus SM_CALL sm_redo(SmHandle handle);
SmStatus SM_CALL sm_get_state(SmHandle handle, SmState *out_state);
SmStatus SM_CALL sm_get_stroke(SmHandle handle, uint64_t revision,
    uint32_t index, SmStrokeInfo *out_info);
SmStatus SM_CALL sm_read_points(SmHandle handle, uint64_t revision,
    uint32_t index, uint32_t start, SmSample *out_points, uint32_t capacity,
    uint32_t *out_written);
SmStatus SM_CALL sm_save(SmHandle handle, uint8_t *out_data,
    uint32_t capacity, uint32_t *out_required);
SmStatus SM_CALL sm_load(SmHandle handle, const uint8_t *data, uint32_t length);
SmStatus SM_CALL sm_read_brush_points(SmHandle handle, uint64_t revision,
    uint32_t index, uint32_t start, const SmBrushOptions *options,
    SmBrushPoint *out_points, uint32_t capacity, uint32_t *out_written);

#ifdef __cplusplus
}
#endif
#endif
