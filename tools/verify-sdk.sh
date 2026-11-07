#!/usr/bin/env bash
# 独立宿主验收：把 SDK tar 解到无源码目录，校验 SHA-256，用 cc 编译冒烟示例并动态加载运行。
# 用法：tools/verify-sdk.sh <sdk.tar> [工作目录]（默认 artifacts/sdk-verify-<随机后缀>）。
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
if [[ $# -lt 1 ]]; then
    echo "用法：tools/verify-sdk.sh <sdk.tar> [工作目录]" >&2
    exit 2
fi
archive=$(cd -- "$(dirname -- "$1")" && pwd)/$(basename -- "$1")
if [[ ! -f $archive ]]; then
    echo "verify-sdk: 找不到包 $archive。" >&2
    exit 1
fi

suffix=$(od -An -N4 -tx4 /dev/urandom | tr -d ' \n')
default_work=$project_root/artifacts/sdk-verify-$suffix
work=${2:-$default_work}
if [[ -e $work ]]; then
    echo "verify-sdk: 工作目录已存在，要求全新目录：$work。" >&2
    exit 1
fi
mkdir -p -- "$work"

manifest=$(tar -tf "$archive")
if grep -qE '(^|/)\.\.(/|$)|^/' <<< "$manifest"; then
    echo "verify-sdk: tar 清单包含绝对路径或 .. 成员，拒绝解包。" >&2
    exit 1
fi
tar -xf "$archive" -C "$work"

tops=$(find "$work" -mindepth 1 -maxdepth 1 | wc -l)
if [[ $tops -ne 1 ]]; then
    echo "verify-sdk: 包内应为单一顶层目录，实际 $tops 项。" >&2
    exit 1
fi
sdk=$(find "$work" -mindepth 1 -maxdepth 1 -type d)

if find "$sdk" \( -name 'Cargo.toml' -o -name '*.rs' \) | grep -q .; then
    echo "verify-sdk: 包内发现引擎源码，验收要求无源码目录。" >&2
    exit 1
fi

target=$(jq -r '.target' "$sdk/build-info.json")
if [[ $target != "x86_64-unknown-linux-gnu" ]]; then
    echo "verify-sdk: 本脚本只验收 x86_64-unknown-linux-gnu，包目标为 $target。" >&2
    exit 1
fi

echo "== checksum =="
(
    cd -- "$sdk"
    sha256sum --check --strict SHA256SUMS
)

echo "== host build =="
cc -std=c11 -Wall -Wextra -Werror -I "$sdk/include" "$sdk/examples/smoke.c" -o "$work/smoke"
ldd "$work/smoke" | tee "$work/ldd-dynamic.txt"
if grep -q libscratchmark "$work/ldd-dynamic.txt"; then
    echo "verify-sdk: 动态宿主不应直接依赖 libscratchmark，示例使用运行时加载。" >&2
    exit 1
fi

echo "== exports =="
# 与 examples/c/smoke.c 的 API_LIST 保持一致；新增导出时同步此清单。
expected_exports=(sm_abi_version sm_features sm_create sm_destroy sm_begin sm_append
    sm_finish sm_cancel sm_undo sm_redo sm_get_state sm_get_stroke sm_read_points
    sm_save sm_load sm_read_brush_points
    sm_get_page_id sm_set_page_id sm_hit_test sm_erase_stroke sm_erase_path sm_lasso_hits sm_erase_indices sm_clear sm_transform_strokes sm_copy_strokes sm_paste sm_regularize_stroke sm_page_to_viewport_ex sm_viewport_to_page_ex sm_width_page_to_viewport_ex sm_width_viewport_to_page_ex sm_bounds_page_to_viewport_ex sm_viewport_ex_zoom_at sm_viewport_ex_fit sm_predict sm_append_axes sm_read_axes sm_document_bounds sm_translate_all
    sm_page_to_viewport sm_viewport_to_page
    sm_width_page_to_viewport sm_width_viewport_to_page sm_bounds_page_to_viewport)
nm -D "$sdk/bin/libscratchmark.so" | tee "$work/exports.txt"
for symbol in "${expected_exports[@]}"; do
    if ! grep -qE " T ${symbol}$" "$work/exports.txt"; then
        echo "verify-sdk: 缺少动态导出 ${symbol}。" >&2
        exit 1
    fi
done
export_count=$(grep -cE ' T sm_' "$work/exports.txt" || true)
if [[ $export_count -ne ${#expected_exports[@]} ]]; then
    echo "verify-sdk: sm_* 导出数量 ${export_count} 与预期 ${#expected_exports[@]} 不符。" >&2
    exit 1
fi

echo "== static host build =="
mkdir -p -- "$work/static"
# 系统库顺序来自 cargo rustc -- --print native-static-libs（Rust 标准库所需）。
cc -std=c11 -Wall -Wextra -Werror -DSM_STATIC -I "$sdk/include" "$sdk/examples/smoke.c" \
    "$sdk/lib/libscratchmark.a" -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc \
    -o "$work/static/smoke-static"
ldd "$work/static/smoke-static" | tee "$work/static/imports.txt"
if grep -q libscratchmark "$work/static/imports.txt"; then
    echo "verify-sdk: 静态宿主不应依赖 libscratchmark 动态库。" >&2
    exit 1
fi

echo "== native smoke =="
"$work/smoke" "$sdk/bin/libscratchmark.so" "$work/roundtrip.scratch"
"$work/static/smoke-static" "$work/static/roundtrip.scratch"
if ! cmp -s "$work/roundtrip.scratch" "$work/static/roundtrip.scratch"; then
    echo "verify-sdk: 动态与静态宿主快照不一致。" >&2
    exit 1
fi
echo "standalone SDK verification passed; checksum, dynamic and static native checks complete"
echo "work dir: $work"
