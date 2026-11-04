param([string]$SdkRoot = $PSScriptRoot, [string]$CCompiler = 'gcc')
$ErrorActionPreference = 'Stop'
$sdk = (Resolve-Path -LiteralPath $SdkRoot).Path
$info = Get-Content -LiteralPath (Join-Path $sdk 'build-info.json') -Raw | ConvertFrom-Json
if ($info.target -ne 'x86_64-pc-windows-gnu') { throw 'Unsupported verification host' }
foreach ($line in Get-Content -LiteralPath (Join-Path $sdk 'SHA256SUMS')) {
    if ($line -notmatch '^([0-9a-f]{64})  (.+)$') { throw 'Invalid checksum record' }
    $expected = $Matches[1]
    $file = [IO.Path]::GetFullPath((Join-Path $sdk $Matches[2]))
    if (-not $file.StartsWith($sdk + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Checksum path outside SDK'
    }
    if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
        throw 'SDK checksum mismatch'
    }
}
$work = Join-Path $sdk ('verification-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Path $work | Out-Null
$exe = Join-Path $work 'smoke.exe'
& $CCompiler -std=c11 -Wall -Wextra -Werror '-I' (Join-Path $sdk 'include') (Join-Path $sdk 'examples/smoke.c') '-o' $exe
if ($LASTEXITCODE -ne 0) { throw 'C host build failed' }
& $exe (Join-Path $sdk 'bin/scratchmark.dll') (Join-Path $work 'roundtrip.scratch')
if ($LASTEXITCODE -ne 0) { throw 'SDK smoke failed' }
$staticWork = Join-Path $work 'static'
New-Item -ItemType Directory -Path $staticWork | Out-Null
$staticExe = Join-Path $staticWork 'smoke-static.exe'
# Rust GNU staticlib 仍需系统库；顺序来自 cargo rustc -- --print native-static-libs。
& $CCompiler -std=c11 -Wall -Wextra -Werror '-DSM_STATIC' '-I' (Join-Path $sdk 'include') (Join-Path $sdk 'examples/smoke.c') (Join-Path $sdk 'lib/libscratchmark.a') '-lkernel32' '-lntdll' '-luserenv' '-lws2_32' '-ldbghelp' '-o' $staticExe
if ($LASTEXITCODE -ne 0) { throw 'Static C host build failed' }
$objdump = & $CCompiler -print-prog-name=objdump
if ($LASTEXITCODE -ne 0) { throw 'objdump discovery failed' }
$imports = & $objdump -p $staticExe
if ($LASTEXITCODE -ne 0) { throw 'Static host import inspection failed' }
$dependencies = @($imports | Select-String 'DLL Name:')
$dependencies | Set-Content -LiteralPath (Join-Path $staticWork 'imports.txt')
if ($dependencies.Count -eq 0) { throw 'No PE import evidence found' }
if ($dependencies -match 'scratchmark.*\.dll') { throw 'Static host depends on engine DLL' }
& $staticExe (Join-Path $staticWork 'roundtrip.scratch')
if ($LASTEXITCODE -ne 0) { throw 'Static SDK smoke failed' }
if ((Get-FileHash -LiteralPath (Join-Path $work 'roundtrip.scratch')).Hash -ne
    (Get-FileHash -LiteralPath (Join-Path $staticWork 'roundtrip.scratch')).Hash) {
    throw 'Dynamic and static host snapshots differ'
}
Write-Output "standalone SDK verification passed; checksum, dynamic and static native checks complete"
Write-Output "verification dir: $work"
