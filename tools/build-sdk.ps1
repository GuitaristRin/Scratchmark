param([string]$OutputRoot = 'dist')
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
Push-Location $projectRoot
try {
    $compilerInfo = & rustc -vV
    if ($LASTEXITCODE -ne 0) { throw 'rustc unavailable' }
    $target = ($compilerInfo | Where-Object { $_ -like 'host: *' }) -replace '^host: ', ''
    if ($target -ne 'x86_64-pc-windows-gnu') {
        throw 'M0 packaging is verified only for x86_64-pc-windows-gnu; no cross-platform claim.'
    }
    & cargo build --release --workspace --locked
    if ($LASTEXITCODE -ne 0) { throw 'release build failed' }
    $metadata = & cargo metadata --no-deps --format-version 1 --locked | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw 'cargo metadata failed' }
    $version = ($metadata.packages | Where-Object { $_.name -eq 'scratchmark-ffi' }).version
    $suffix = [guid]::NewGuid().ToString('N').Substring(0, 8)
    $name = "scratchmark-$version-$target-$suffix"
    $output = [IO.Path]::GetFullPath((Join-Path $projectRoot $OutputRoot))
    $sdk = Join-Path $output $name
    New-Item -ItemType Directory -Path $sdk -ErrorAction Stop | Out-Null
    foreach ($dir in @('bin', 'lib', 'include', 'examples', 'spec')) {
        New-Item -ItemType Directory -Path (Join-Path $sdk $dir) | Out-Null
    }
    Copy-Item -LiteralPath 'target/release/scratchmark.dll' -Destination (Join-Path $sdk 'bin')
    Copy-Item -LiteralPath 'target/release/libscratchmark.a' -Destination (Join-Path $sdk 'lib')
    Copy-Item -LiteralPath 'include/scratchmark.h' -Destination (Join-Path $sdk 'include')
    Copy-Item -LiteralPath 'examples/c/smoke.c' -Destination (Join-Path $sdk 'examples')
    Copy-Item -LiteralPath 'spec/M0.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'docs/SDK_README.md' -Destination (Join-Path $sdk 'README.md')
    Copy-Item -LiteralPath 'tools/verify-sdk.ps1' -Destination (Join-Path $sdk 'verify-sdk.ps1')
    $head = & git rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'git identity unavailable' }
    $dirty = @(& git status --porcelain --untracked-files=normal).Count -gt 0
    if ($LASTEXITCODE -ne 0) { throw 'git status failed' }
    [ordered]@{
        sdkVersion = $version; abi = 1; snapshotFormat = 1; target = $target
        sourceCommit = $head; sourceDirty = $dirty; compiler = ($compilerInfo -join "`n")
        status = 'experimental'; dependencies = 'Rust standard library and Windows system libraries'
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $sdk 'build-info.json') -Encoding UTF8
    $hashes = foreach ($file in Get-ChildItem -LiteralPath $sdk -File -Recurse | Sort-Object FullName) {
        $relative = $file.FullName.Substring($sdk.Length + 1).Replace('\', '/')
        '{0}  {1}' -f (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant(), $relative
    }
    $hashes | Set-Content -LiteralPath (Join-Path $sdk 'SHA256SUMS') -Encoding ASCII
    $archive = "$sdk.zip"
    Compress-Archive -LiteralPath $sdk -DestinationPath $archive -ErrorAction Stop
    Write-Output $sdk
    Write-Output $archive
} finally { Pop-Location }
