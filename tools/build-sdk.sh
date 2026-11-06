#!/usr/bin/env bash
# 构建 Linux x86_64 SDK：动态库、静态库（如可用）、头文件、规格与冒烟示例，打包为 tar 并校验清单。
# 用法：tools/build-sdk.sh [输出根目录]（相对工程根解析，默认 dist）。
set -euo pipefail

project_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$project_root"

compiler_info=$(rustc -vV)
target=$(awk '/^host: / { print $2 }' <<< "$compiler_info")
if [[ $target != "x86_64-unknown-linux-gnu" ]]; then
    echo "build-sdk: M0 打包只针对 x86_64-unknown-linux-gnu，不做跨平台声明（host=$target）。" >&2
    exit 1
fi

cargo build --release --workspace --locked
version=$(cargo metadata --no-deps --format-version 1 --locked | jq -r '.packages[] | select(.name == "scratchmark-ffi") | .version')

so_path=target/release/libscratchmark.so
a_path=target/release/libscratchmark.a
if [[ ! -f $so_path ]]; then
    echo "build-sdk: 缺少动态库产物 $so_path。" >&2
    exit 1
fi

suffix=$(od -An -N4 -tx4 /dev/urandom | tr -d ' \n')
name="scratchmark-$version-$target-$suffix"
output=${1:-dist}
mkdir -p -- "$output"
output=$(cd -- "$output" && pwd)
sdk=$output/$name
mkdir -p -- "$sdk/bin" "$sdk/include" "$sdk/examples" "$sdk/spec"

cp -- "$so_path" "$sdk/bin/"
if [[ -f $a_path ]]; then
    mkdir -p -- "$sdk/lib"
    cp -- "$a_path" "$sdk/lib/"
fi
cp -- include/scratchmark.h "$sdk/include/"
cp -- examples/c/smoke.c "$sdk/examples/"
cp -- spec/M0.md "$sdk/spec/"
cp -- spec/M2.md "$sdk/spec/"
cp -- spec/M3.md "$sdk/spec/"
cp -- spec/M4.md "$sdk/spec/"
cp -- spec/M5.md "$sdk/spec/"
cp -- spec/M6.md "$sdk/spec/"

head=$(git rev-parse HEAD)
if [[ -n $(git status --porcelain --untracked-files=normal) ]]; then dirty=true; else dirty=false; fi
jq -n \
    --arg sdkVersion "$version" \
    --arg target "$target" \
    --arg sourceCommit "$head" \
    --argjson sourceDirty "$dirty" \
    --arg compiler "$compiler_info" \
    --arg dependencies "Rust standard library and Linux system libraries" \
    '{sdkVersion: $sdkVersion, abi: 1, features: 31, snapshotFormat: 2, target: $target,
      sourceCommit: $sourceCommit, sourceDirty: $sourceDirty, compiler: $compiler,
      status: "experimental", dependencies: $dependencies}' > "$sdk/build-info.json"

(
    cd -- "$sdk"
    find . -type f ! -name SHA256SUMS -printf '%P\0' | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
)

archive=$output/$name.tar
tar -cf "$archive" -C "$output" "$name"
tar -tf "$archive" > /dev/null
echo "$sdk"
echo "$archive"
echo "tar manifest:"
tar -tf "$archive"
