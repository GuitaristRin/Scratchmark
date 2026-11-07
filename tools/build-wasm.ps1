# 构建 WebAssembly 产物并在 Node 里跑冒烟断言。
#
# 验证等级：**真跑**。Node 直接实例化 wasm 并调用 C ABI，见 tools/wasm-smoke.mjs。
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

Push-Location $root
try {
    Write-Host '== 构建 wasm32-unknown-unknown'
    & cargo build -p scratchmark-ffi --release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw 'cargo build 失败' }

    $wasm = Join-Path $root 'target/wasm32-unknown-unknown/release/scratchmark.wasm'
    if (-not (Test-Path $wasm)) { throw "产物缺失: $wasm" }
    $size = (Get-Item $wasm).Length
    Write-Host ("   产物 {0:N0} 字节" -f $size)

    Write-Host '== Node 冒烟'
    & node (Join-Path $root 'tools/wasm-smoke.mjs') $wasm
    if ($LASTEXITCODE -ne 0) { throw 'wasm 冒烟失败' }

    $out = Join-Path $root 'dist/wasm'
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    Copy-Item $wasm (Join-Path $out 'scratchmark.wasm') -Force
    Write-Host "WASM 产物: dist/wasm/scratchmark.wasm"
}
finally {
    Pop-Location
}
