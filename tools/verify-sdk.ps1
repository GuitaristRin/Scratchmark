param([string]$SdkRoot = $PSScriptRoot, [string]$CCompiler = 'gcc', [switch]$IncludeCSharp)
$ErrorActionPreference = 'Stop'
$sdk = (Resolve-Path -LiteralPath $SdkRoot).Path
$info = Get-Content -LiteralPath (Join-Path $sdk 'build-info.json') -Raw | ConvertFrom-Json
if ($info.target -ne 'x86_64-pc-windows-gnu') { throw 'Unsupported verification host' }
if (Get-ChildItem -LiteralPath $sdk -Recurse -File | Where-Object { $_.Extension -eq '.rs' -or $_.Name -eq 'Cargo.toml' }) {
    throw 'Standalone SDK must not contain engine source'
}
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
& $CCompiler -std=c11 -Wall -Wextra -Werror '-I' (Join-Path $sdk 'include') (Join-Path $sdk 'examples/smoke.c') '-o' $exe 2>&1
if ($LASTEXITCODE -ne 0) { throw 'C host build failed' }
& $exe (Join-Path $sdk 'bin/scratchmark.dll') (Join-Path $work 'roundtrip.scratch')
if ($LASTEXITCODE -ne 0) { throw 'SDK smoke failed' }
$staticWork = Join-Path $work 'static'
New-Item -ItemType Directory -Path $staticWork | Out-Null
$staticExe = Join-Path $staticWork 'smoke-static.exe'
# Rust GNU staticlib 仍需系统库；顺序来自 cargo rustc -- --print native-static-libs。
$staticCompile = & $CCompiler -std=c11 -Wall -Wextra -Werror '-DSM_STATIC' '-I' (Join-Path $sdk 'include') (Join-Path $sdk 'examples/smoke.c') (Join-Path $sdk 'lib/libscratchmark.a') '-lkernel32' '-lntdll' '-luserenv' '-lws2_32' '-ldbghelp' '-o' $staticExe 2>&1
$staticCompile | Write-Output
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
if ($IncludeCSharp) {
    $managed = Join-Path $work 'managed'
    $project = Join-Path $sdk 'examples/csharp/Scratchmark.Smoke.csproj'
    $environment = @{
        DOTNET_CLI_HOME = (Join-Path $work 'dotnet-home')
        DOTNET_CLI_TELEMETRY_OPTOUT = '1'; DOTNET_SKIP_FIRST_TIME_EXPERIENCE = '1'
        DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'; DOTNET_ADD_GLOBAL_TOOLS_TO_PATH = 'false'
        DOTNET_CLI_WORKLOAD_UPDATE_NOTIFY_DISABLE = 'true'
    }
    $previous = @{}
    try {
        foreach ($key in $environment.Keys) {
            $previous[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
            [Environment]::SetEnvironmentVariable($key, $environment[$key], 'Process')
        }
        & dotnet restore $project --configfile (Join-Path $sdk 'bindings/csharp/NuGet.Config') --packages (Join-Path $work 'packages')
        if ($LASTEXITCODE -ne 0) { throw 'C# offline restore failed' }
        & dotnet build $project --no-restore -c Release -o $managed | Tee-Object -FilePath (Join-Path $work 'managed-build.txt')
        if ($LASTEXITCODE -ne 0) { throw 'C# host build failed' }
        & dotnet (Join-Path $managed 'Scratchmark.Smoke.dll') (Join-Path $sdk 'bin/scratchmark.dll') (Join-Path $sdk 'spec/fixtures/simple-v1.hex') (Join-Path $managed 'roundtrip.scratch') | Tee-Object -FilePath (Join-Path $work 'managed-smoke.txt')
        if ($LASTEXITCODE -ne 0) { throw 'C# smoke failed' }
        if ((Get-FileHash -LiteralPath (Join-Path $work 'roundtrip.scratch')).Hash -ne
            (Get-FileHash -LiteralPath (Join-Path $managed 'roundtrip.scratch')).Hash) {
            throw 'Native and C# host snapshots differ'
        }
        Write-Output 'C# standalone verification passed; snapshot matches native host and shared fixture'
    } finally {
        foreach ($key in $previous.Keys) { [Environment]::SetEnvironmentVariable($key, $previous[$key], 'Process') }
    }
}
Write-Output "verification dir: $work"
