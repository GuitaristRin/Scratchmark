# 构建 Linux 目标；本机只能出静态库（musl 不支持 cdylib），见 docs/PLATFORMS.md。
#
# 验证等级：仅编译与符号核对——本机没有 Linux 运行时。
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$target = 'x86_64-unknown-linux-musl'

Push-Location $root
try {
    Write-Host "== 构建 $target"
    & cargo build -p scratchmark-ffi --release --target $target
    if ($LASTEXITCODE -ne 0) { throw 'cargo build 失败' }

    $lib = Join-Path $root "target/$target/release/libscratchmark.a"
    if (-not (Test-Path $lib)) { throw "产物缺失: $lib" }
    Write-Host ("   libscratchmark.a {0:N0} 字节（静态库；musl 不支持 cdylib）" -f (Get-Item $lib).Length)

    $out = Join-Path $root 'dist/linux'
    New-Item -ItemType Directory -Force -Path $out | Out-Null
    Copy-Item $lib (Join-Path $out 'libscratchmark.a') -Force
    Write-Host 'Linux 产物: dist/linux/libscratchmark.a'
    Write-Host '要产出 .so 需要 Linux 链接器：cargo build -p scratchmark-ffi --release --target x86_64-unknown-linux-gnu'
}
finally {
    Pop-Location
}
