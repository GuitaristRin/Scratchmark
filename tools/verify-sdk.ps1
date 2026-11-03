param([string]$SdkRoot = $PSScriptRoot)
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
& clang -std=c11 -Wall -Wextra -Werror '-I' (Join-Path $sdk 'include') (Join-Path $sdk 'examples/smoke.c') '-o' $exe
if ($LASTEXITCODE -ne 0) { throw 'C host build failed' }
& $exe (Join-Path $sdk 'bin/scratchmark.dll') (Join-Path $work 'roundtrip.scratch')
if ($LASTEXITCODE -ne 0) { throw 'SDK smoke failed' }
Write-Output "standalone SDK verification passed; checksum and native checks complete"
