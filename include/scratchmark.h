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
/* M3 页面标识与视口；映射、页面域与快照格式参 spec/M3.md。 */
#define SM_FEATURE_PAGE_VIEWPORT UINT64_C(2)
#define SM_FEATURE_HIT_ERASE UINT64_C(4)
/* M5 局部擦除；路径、半径与切分语义参 spec/M5.md。 */
#define SM_FEATURE_PARTIAL_ERASE UINT64_C(8)

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

/* M3 视口是宿主保留的显示配置，不入快照；旋转 / DPI 处理参 spec/M3.md。 */
typedef struct SmViewport {
    double scale, offset_x, offset_y;
    uint32_t reserved;
} SmViewport;
typedef struct SmPoint {
    double x, y;
} SmPoint;
typedef struct SmBounds {
    double min_x, min_y, max_x, max_y;
} SmBounds;
/* 页面标识为宿主选择的稳定键，0 表示未设置。 */
SmStatus SM_CALL sm_get_page_id(SmHandle handle, uint64_t *out_page_id);
SmStatus SM_CALL sm_set_page_id(SmHandle handle, uint64_t page_id);
SmStatus SM_CALL sm_hit_test(SmHandle handle, uint64_t revision,
    double x, double y, double tolerance,
    uint32_t *out_index, uint32_t *out_found);
SmStatus SM_CALL sm_erase_stroke(SmHandle handle, uint64_t revision,
    uint32_t index);
SmStatus SM_CALL sm_erase_path(SmHandle handle, uint64_t revision,
    const SmPoint *path, uint32_t count, double radius);
/* 视口 <-> 页面纯函数：viewport = page * scale + offset；线宽按 scale 缩放。 */
SmStatus SM_CALL sm_page_to_viewport(const SmViewport *viewport,
    const SmPoint *in, SmPoint *out);
SmStatus SM_CALL sm_viewport_to_page(const SmViewport *viewport,
    const SmPoint *in, SmPoint *out);
SmStatus SM_CALL sm_width_page_to_viewport(const SmViewport *viewport,
    double width, double *out_width);
SmStatus SM_CALL sm_width_viewport_to_page(const SmViewport *viewport,
    double width, double *out_width);
SmStatus SM_CALL sm_bounds_page_to_viewport(const SmViewport *viewport,
    const SmBounds *in, SmBounds *out);

#ifdef __cplusplus
}
#endif
#endif
