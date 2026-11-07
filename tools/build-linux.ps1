# 构建 Linux 交付物。
#
# 有 zig：用 zig cc 作交叉链接器产 **x86_64-linux-gnu 的 .so**（glibc 2.17 基线），核对符号与依赖后 strip。
# 没有 zig：退回 musl 静态库（cargo 会丢弃 musl 的 cdylib，只能出 .a）。
#
# 验证等级：编译 + 符号 + 依赖核对；本机没有 Linux 运行时，**不验证运行**。
param(
    [string]$OutDir = 'dist/linux'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$zig = $env:ZIG_EXE
if (-not $zig) {
    $candidate = Get-ChildItem (Join-Path $env:LOCALAPPDATA 'zig') -Recurse -Filter 'zig.exe' -ErrorAction SilentlyContinue |
    Select-Object -First 1
    if ($candidate) { $zig = $candidate.FullName }
    elseif (Get-Command zig -ErrorAction SilentlyContinue) { $zig = (Get-Command zig).Source }
}

$outPath = Join-Path $root $OutDir
New-Item -ItemType Directory -Force -Path $outPath | Out-Null

if (-not $zig) {
    Write-Host '== 没有 zig：只能构建 musl 静态库'
    Push-Location $root
    try {
        & cargo build -p scratchmark-ffi --release --target x86_64-unknown-linux-musl
        if ($LASTEXITCODE -ne 0) { throw 'cargo build 失败' }
        Copy-Item (Join-Path $root 'target/x86_64-unknown-linux-musl/release/libscratchmark.a') `
            (Join-Path $outPath 'libscratchmark.a') -Force
        Write-Host '产物: dist/linux/libscratchmark.a（静态库；要 .so 请安装 zig 或使用 Linux 链接器）'
    }
    finally { Pop-Location }
    exit 0
}

Write-Host "== 使用 zig: $zig"
& $zig version | ForEach-Object { Write-Host "   zig $_" }

# 运行时生成链接器包装：避免把某台机器的绝对路径提交进仓库。
$wrapper = Join-Path $outPath 'zigcc.cmd'
@"
@echo off
"$zig" cc -target x86_64-linux-gnu.2.17 %*
"@ | Set-Content -Path $wrapper -Encoding ascii

Push-Location $root
try {
    $env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = $wrapper
    Write-Host '== 构建 x86_64-unknown-linux-gnu'
    & cargo build -p scratchmark-ffi --release --target x86_64-unknown-linux-gnu
    if ($LASTEXITCODE -ne 0) { throw 'cargo build 失败' }

    $so = Join-Path $root 'target/x86_64-unknown-linux-gnu/release/libscratchmark.so'
    if (-not (Test-Path $so)) { throw "产物缺失: $so" }

    # 用 NDK 自带的 llvm 工具读 ELF（没有 NDK 就跳过核对，并在输出里说明）。
    $ndkBin = $null
    $ndk = Get-ChildItem 'D:\Applications\AndroidSDK\ndk' -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name | Select-Object -Last 1
    if ($ndk) { $ndkBin = Join-Path $ndk.FullName 'toolchains/llvm/prebuilt/windows-x86_64/bin' }
    if (-not $ndkBin -or -not (Test-Path (Join-Path $ndkBin 'llvm-nm.exe'))) {
        Write-Host '   没有 llvm 工具，跳过符号核对'
    }
    else {
        $header = & (Join-Path $ndkBin 'llvm-readelf.exe') -h $so 2>&1
        $type = ($header | Select-String -Pattern 'Type:\s+(\S+)').Matches[0].Groups[1].Value
        $machine = ($header | Select-String -Pattern 'Machine:\s+(.+)').Matches[0].Groups[1].Value.Trim()
        if ($type -notmatch 'DYN') { throw "不是共享库: Type=$type" }
        Write-Host "   ELF: $machine / $type"

        $symbols = & (Join-Path $ndkBin 'llvm-nm.exe') --defined-only --extern-only $so 2>&1
        $exported = ($symbols | Select-String -Pattern '\bT (sm_\w+)' -AllMatches |
            ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value }) | Sort-Object -Unique
        $required = @('sm_create', 'sm_append', 'sm_save', 'sm_alloc', 'sm_free',
            'sm_cut_strokes', 'sm_resample_stroke', 'sm_predict_ex', 'sm_document_bounds')
        $missing = $required | Where-Object { $exported -notcontains $_ }
        if ($missing) { throw "缺少导出: $($missing -join ', ')" }
        Write-Host "   导出: $($exported.Count) 个 sm_* 符号"

        $needed = & (Join-Path $ndkBin 'llvm-readelf.exe') -d $so 2>&1 |
        Select-String -Pattern 'NEEDED.*\[(.+)\]' | ForEach-Object { $_.Matches[0].Groups[1].Value }
        Write-Host "   依赖: $($needed -join ', ')"

        & (Join-Path $ndkBin 'llvm-strip.exe') --strip-unneeded $so
    }

    Copy-Item $so (Join-Path $outPath 'libscratchmark.so') -Force
    $size = (Get-Item (Join-Path $outPath 'libscratchmark.so')).Length
    Write-Host ("产物: dist/linux/libscratchmark.so（{0:N0} 字节，glibc 2.17 基线）" -f $size)
    Write-Host '注意：只验证了编译、符号与依赖；本机无 Linux 运行时。'
}
finally {
    Pop-Location
    $env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = $null
}
