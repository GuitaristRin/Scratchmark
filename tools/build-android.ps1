# 交叉编译 Android 三个 ABI 的 libscratchmark.so，并用 NDK 自带工具核对导出与依赖。
#
# 验证等级：**可编译、符号与依赖可核对**；本机没有设备/模拟器，无法运行验证（见 docs/PLATFORMS.md）。
# 用法：pwsh tools/build-android.ps1 [-Api 24] [-OutDir dist/android]
param(
    [int]$Api = 24,
    [string]$OutDir = 'dist/android'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$ndk = if ($env:ANDROID_NDK_HOME) { $env:ANDROID_NDK_HOME }
elseif ($env:ANDROID_NDK_ROOT) { $env:ANDROID_NDK_ROOT }
else {
    $candidate = Get-ChildItem 'D:\Applications\AndroidSDK\ndk' -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name | Select-Object -Last 1
    if (-not $candidate) { throw 'NDK 未找到：设置 ANDROID_NDK_HOME 或安装到 D:\Applications\AndroidSDK\ndk' }
    $candidate.FullName
}

$hostTag = if ($IsWindows) { 'windows-x86_64' } elseif ($IsMacOS) { 'darwin-x86_64' } else { 'linux-x86_64' }
$bin = Join-Path $ndk "toolchains/llvm/prebuilt/$hostTag/bin"
if (-not (Test-Path $bin)) { throw "NDK 工具链不存在: $bin" }

$suffix = if ($IsWindows) { '.cmd' } else { '' }
$targets = @(
    @{ Rust = 'aarch64-linux-android'; Clang = "aarch64-linux-android$Api-clang"; Abi = 'arm64-v8a' },
    @{ Rust = 'armv7-linux-androideabi'; Clang = "armv7a-linux-androideabi$Api-clang"; Abi = 'armeabi-v7a' },
    @{ Rust = 'x86_64-linux-android'; Clang = "x86_64-linux-android$Api-clang"; Abi = 'x86_64' }
)

$expected = @(
    'sm_abi_version', 'sm_features', 'sm_create', 'sm_destroy', 'sm_begin', 'sm_append', 'sm_finish',
    'sm_cancel', 'sm_undo', 'sm_redo', 'sm_save', 'sm_load', 'sm_copy_strokes', 'sm_paste',
    'sm_erase_path', 'sm_document_bounds', 'sm_translate_all', 'sm_alloc', 'sm_free'
)

$ndkVersion = Split-Path -Leaf $ndk
Write-Host "NDK: $ndk ($ndkVersion), API $Api"

foreach ($target in $targets) {
    $clang = Join-Path $bin ($target.Clang + $suffix)
    if (-not (Test-Path $clang)) { throw "找不到交叉编译器: $clang" }

    $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = $null
    $envName = 'CARGO_TARGET_' + ($target.Rust.ToUpper().Replace('-', '_')) + '_LINKER'
    Set-Item -Path "Env:$envName" -Value $clang
    Set-Item -Path "Env:CC_$($target.Rust.Replace('-', '_'))" -Value $clang

    Write-Host "== 构建 $($target.Rust) ($($target.Abi))"
    & cargo build -p scratchmark-ffi --release --target $target.Rust
    if ($LASTEXITCODE -ne 0) { throw "cargo build 失败: $($target.Rust)" }

    $so = Join-Path $root "target/$($target.Rust)/release/libscratchmark.so"
    if (-not (Test-Path $so)) { throw "产物缺失: $so" }

    Write-Host "== 构建 JNI 封装 $($target.Rust)"
    & cargo build -p scratchmark-jni --release --target $target.Rust
    if ($LASTEXITCODE -ne 0) { throw "JNI cargo build 失败: $($target.Rust)" }
    $jniSo = Join-Path $root "target/$($target.Rust)/release/libscratchmark_jni.so"
    if (-not (Test-Path $jniSo)) { throw "JNI 产物缺失: $jniSo" }

    $abiDir = Join-Path $root "$OutDir/jniLibs/$($target.Abi)"
    New-Item -ItemType Directory -Force -Path $abiDir | Out-Null
    Copy-Item $so (Join-Path $abiDir 'libscratchmark.so') -Force
    Copy-Item $jniSo (Join-Path $abiDir 'libscratchmark_jni.so') -Force

    $nm = Join-Path $bin 'llvm-nm.exe'
    if (-not (Test-Path $nm)) { $nm = Join-Path $bin 'llvm-nm' }
    $strip = Join-Path $bin 'llvm-strip.exe'
    if (-not (Test-Path $strip)) { $strip = Join-Path $bin 'llvm-strip' }

    # 先核对再 strip：strip 不改变导出符号，但先核对能给出更清楚的失败信息。
    $symbols = & $nm --defined-only --extern-only (Join-Path $abiDir 'libscratchmark.so') 2>&1
    $exported = ($symbols | Select-String -Pattern '\bT (sm_\w+)' -AllMatches |
        ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value }) | Sort-Object -Unique
    $missing = $expected | Where-Object { $exported -notcontains $_ }
    if ($missing) { throw "$($target.Abi) 缺少导出: $($missing -join ', ')" }

    $jniSymbols = & $nm --defined-only --extern-only (Join-Path $abiDir 'libscratchmark_jni.so') 2>&1
    $jniExported = ($jniSymbols | Select-String -Pattern '\bT (Java_\w+)' -AllMatches |
        ForEach-Object { $_.Matches } | ForEach-Object { $_.Groups[1].Value }) | Sort-Object -Unique
    $required = @('Java_com_scratchmark_Native_create', 'Java_com_scratchmark_Native_append',
        'Java_com_scratchmark_Native_save', 'Java_com_scratchmark_Native_erasePath',
        'Java_com_scratchmark_Native_brushPoints', 'Java_com_scratchmark_Native_cutStrokes')
    $jniMissing = $required | Where-Object { $jniExported -notcontains $_ }
    if ($jniMissing) { throw "$($target.Abi) 的 JNI 封装缺少导出: $($jniMissing -join ', ')" }

    # 去掉符号与调试信息：手机上 5 MB/ABI 太重。
    & $strip --strip-unneeded (Join-Path $abiDir 'libscratchmark.so')
    & $strip --strip-unneeded (Join-Path $abiDir 'libscratchmark_jni.so')

    $size = (Get-Item (Join-Path $abiDir 'libscratchmark.so')).Length
    $jniSize = (Get-Item (Join-Path $abiDir 'libscratchmark_jni.so')).Length
    Write-Host ("   ok: {0} 个 C 导出 / {1} 个 JNI 导出，{2:N0} + {3:N0} 字节" -f `
            $exported.Count, $jniExported.Count, $size, $jniSize)
}

Write-Host "Android 产物: $OutDir/jniLibs/{arm64-v8a,armeabi-v7a,x86_64}/{libscratchmark,libscratchmark_jni}.so"
Write-Host "注意：只验证了编译与导出符号；本机无设备，未验证运行时。"
