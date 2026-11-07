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
    foreach ($dir in @('bin', 'lib', 'include', 'examples', 'spec', 'examples/csharp', 'bindings/csharp/Scratchmark', 'spec/fixtures')) {
        New-Item -ItemType Directory -Path (Join-Path $sdk $dir) | Out-Null
    }
    Copy-Item -LiteralPath 'target/release/scratchmark.dll' -Destination (Join-Path $sdk 'bin')
    Copy-Item -LiteralPath 'target/release/libscratchmark.a' -Destination (Join-Path $sdk 'lib')
    Copy-Item -LiteralPath 'include/scratchmark.h' -Destination (Join-Path $sdk 'include')
    Copy-Item -LiteralPath 'examples/c/smoke.c' -Destination (Join-Path $sdk 'examples')
    Copy-Item -LiteralPath 'spec/M0.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M1-CSHARP.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M2.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M3.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M4.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M5.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M6.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M7.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M8.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M9.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M10.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/M11.md' -Destination (Join-Path $sdk 'spec')
    Copy-Item -LiteralPath 'spec/fixtures/simple-v1.hex' -Destination (Join-Path $sdk 'spec/fixtures')
    Copy-Item -LiteralPath 'spec/fixtures/simple-v2-page42.hex' -Destination (Join-Path $sdk 'spec/fixtures')
    Copy-Item -LiteralPath 'bindings/csharp/NuGet.Config' -Destination (Join-Path $sdk 'bindings/csharp')
    foreach ($file in @('Scratchmark.csproj', 'Types.cs', 'Native.cs', 'Engine.cs')) {
        Copy-Item -LiteralPath (Join-Path 'bindings/csharp/Scratchmark' $file) -Destination (Join-Path $sdk 'bindings/csharp/Scratchmark')
    }
    foreach ($file in @('Scratchmark.Smoke.csproj', 'Program.cs', 'NativeErrors.cs', 'Brush.cs', 'PageViewport.cs', 'HitErase.cs', 'LocalErase.cs', 'Selection.cs', 'TransformSelection.cs', 'ShapeRegularize.cs', 'ViewportRotation.cs', 'Prediction.cs', 'StylusAxes.cs')) {
        Copy-Item -LiteralPath (Join-Path 'examples/csharp' $file) -Destination (Join-Path $sdk 'examples/csharp')
    }
    Copy-Item -LiteralPath 'docs/SDK_README.md' -Destination (Join-Path $sdk 'README.md')
    Copy-Item -LiteralPath 'docs/HOST_INTEGRATION.md' -Destination (Join-Path $sdk 'HOST_INTEGRATION.md')
    Copy-Item -LiteralPath 'tools/verify-sdk.ps1' -Destination (Join-Path $sdk 'verify-sdk.ps1')
    $head = & git rev-parse HEAD
    if ($LASTEXITCODE -ne 0) { throw 'git identity unavailable' }
    $dirty = @(& git status --porcelain --untracked-files=normal).Count -gt 0
    if ($LASTEXITCODE -ne 0) { throw 'git status failed' }
    [ordered]@{
        sdkVersion = $version; abi = 1; features = 1023; snapshotFormat = 2; target = $target
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
    & tar -tf $archive | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'SDK archive listing failed' }
    Write-Output $sdk
    Write-Output $archive
} finally { Pop-Location }
