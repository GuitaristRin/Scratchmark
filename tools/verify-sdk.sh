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
ldd "$work/smoke"

echo "== native smoke =="
"$work/smoke" "$sdk/bin/libscratchmark.so" "$work/roundtrip.scratch"
echo "standalone SDK verification passed; checksum and native checks complete"
echo "work dir: $work"
