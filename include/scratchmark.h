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

/* M2 派生几何；参数、采样滞后与宿主配置保存责任参 spec/M2.md。 */
typedef struct SmBrushOptions {
    double alpha, min_width_ratio;
    uint32_t pressure_enabled, reserved;
} SmBrushOptions;
typedef struct SmBrushPoint {
    double x, y, width;
} SmBrushPoint;
#define SM_FEATURE_BRUSH_GEOMETRY UINT64_C(1)
SmStatus SM_CALL sm_read_brush_points(SmHandle handle, uint64_t revision,
    uint32_t index, uint32_t start, const SmBrushOptions *options,
    SmBrushPoint *out_points, uint32_t capacity, uint32_t *out_written);

/* M3 页面标识与视口；映射、页面域与快照格式参 spec/M3.md。
   视口是宿主保留的显示配置，不入快照；DPI 计入 scale。 */
#define SM_FEATURE_PAGE_VIEWPORT UINT64_C(2)
#define SM_FEATURE_HIT_ERASE UINT64_C(4)
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

/* M4 命中测试与整笔擦除；语义参 spec/M4.md。 */
SmStatus SM_CALL sm_hit_test(SmHandle handle, uint64_t revision,
    double x, double y, double tolerance,
    uint32_t *out_index, uint32_t *out_found);
SmStatus SM_CALL sm_erase_stroke(SmHandle handle, uint64_t revision,
    uint32_t index);

/* M5 局部擦除；路径、半径与切分语义参 spec/M5.md。 */
#define SM_FEATURE_PARTIAL_ERASE UINT64_C(8)
SmStatus SM_CALL sm_erase_path(SmHandle handle, uint64_t revision,
    const SmPoint *path, uint32_t count, double radius);

/* M6 套索命中与批量删除；多边形判定与索引规则参 spec/M6.md。
   套索命中是只读查询；引擎不保存选区，高亮与选中态由宿主保留。 */
#define SM_FEATURE_SELECTION UINT64_C(16)
SmStatus SM_CALL sm_lasso_hits(SmHandle handle, uint64_t revision,
    const SmPoint *path, uint32_t count,
    uint32_t *out_indices, uint32_t capacity, uint32_t *out_written);
SmStatus SM_CALL sm_erase_indices(SmHandle handle, uint64_t revision,
    const uint32_t *indices, uint32_t count);
SmStatus SM_CALL sm_clear(SmHandle handle, uint64_t revision);

/* M7 选区变换与复制粘贴；矩阵语义与剪贴板格式参 spec/M7.md。
   复制产出格式 1 快照字节，粘贴只接受该格式。 */
#define SM_FEATURE_TRANSFORM UINT64_C(32)
typedef struct SmTransform {
    double m00, m01, m10, m11, dx, dy;
} SmTransform;
SmStatus SM_CALL sm_transform_strokes(SmHandle handle, uint64_t revision,
    const uint32_t *indices, uint32_t count, const SmTransform *transform);
SmStatus SM_CALL sm_copy_strokes(SmHandle handle, uint64_t revision,
    const uint32_t *indices, uint32_t count,
    uint8_t *out_data, uint32_t capacity, uint32_t *out_required);
SmStatus SM_CALL sm_paste(SmHandle handle, uint64_t revision,
    const uint8_t *data, uint32_t length, const SmTransform *transform);

/* M8 形状规整；种类、残差与预览模式参 spec/M8.md。
   flags bit 0 = 只测量不替换（预览）；残差由宿主决定是否采纳。 */
#define SM_FEATURE_SHAPE UINT64_C(64)
enum {
    SM_SHAPE_LINE = 1, SM_SHAPE_RECTANGLE = 2, SM_SHAPE_CIRCLE = 3, SM_SHAPE_ARROW = 4
};
typedef struct SmShapeInfo {
    uint32_t kind, stroke_count, point_count, reserved;
    double error;
} SmShapeInfo;
SmStatus SM_CALL sm_regularize_stroke(SmHandle handle, uint64_t revision,
    uint32_t index, uint32_t kind, uint32_t flags, SmShapeInfo *out_info);

/* M9 带旋转的视口与视口辅助；语义参 spec/M9.md。
   旧 SmViewport 与下面五个旧导出等价于 angle = 0，语义不变。 */
#define SM_FEATURE_VIEWPORT_ROTATION UINT64_C(128)
typedef struct SmViewportEx {
    double scale, angle, offset_x, offset_y;
    uint32_t reserved;
} SmViewportEx;
SmStatus SM_CALL sm_page_to_viewport_ex(const SmViewportEx *viewport,
    const SmPoint *in, SmPoint *out);
SmStatus SM_CALL sm_viewport_to_page_ex(const SmViewportEx *viewport,
    const SmPoint *in, SmPoint *out);
SmStatus SM_CALL sm_width_page_to_viewport_ex(const SmViewportEx *viewport,
    double width, double *out_width);
SmStatus SM_CALL sm_width_viewport_to_page_ex(const SmViewportEx *viewport,
    double width, double *out_width);
SmStatus SM_CALL sm_bounds_page_to_viewport_ex(const SmViewportEx *viewport,
    const SmBounds *in, SmBounds *out);
/* 以视口点为锚缩放：锚点下的页面点保持不变。 */
SmStatus SM_CALL sm_viewport_ex_zoom_at(const SmViewportEx *viewport,
    const SmPoint *anchor, double factor, SmViewportEx *out_viewport);
/* 让页面边界适配视口矩形：按旋转后的外接框等比缩放并居中。 */
SmStatus SM_CALL sm_viewport_ex_fit(const SmBounds *page_bounds,
    double view_width, double view_height, double angle, double padding,
    SmViewportEx *out_viewport);

/* M11 逐点轴向（倾斜 / 朝向）；值域、传播与格式 3 参 spec/M11.md。 */
#define SM_FEATURE_STYLUS_AXES UINT64_C(512)
typedef struct SmSampleAxes {
    float tilt;        /* 弧度，[0, π/2]，0 表示垂直或未提供 */
    float orientation; /* 弧度，[-π, π] */
} SmSampleAxes;
SmStatus SM_CALL sm_append_axes(SmHandle handle, const SmSample *samples,
    const SmSampleAxes *axes, uint32_t count);
SmStatus SM_CALL sm_read_axes(SmHandle handle, uint64_t revision, uint32_t index,
    uint32_t start, SmSampleAxes *out_axes, uint32_t capacity, uint32_t *out_written);

/* M10 预测笔迹；模型、上限与宿主责任参 spec/M10.md。预测不是文档内容。 */
#define SM_FEATURE_PREDICTION UINT64_C(256)
typedef struct SmPredictionOptions {
    double horizon_us, max_distance;
    uint32_t count, reserved;
    SmBrushOptions brush;
} SmPredictionOptions;
SmStatus SM_CALL sm_predict(SmHandle handle, uint64_t revision,
    const SmPredictionOptions *options,
    SmBrushPoint *out_points, uint32_t capacity, uint32_t *out_written);
#ifdef __cplusplus
}
#endif
#endif
