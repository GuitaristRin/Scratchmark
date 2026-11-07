# 打 NuGet 包并验证：包内按 RID 放原生库，消费方用无参 Engine() 解析。
#
# 验证等级：**本机真跑**（win-x64）——解包到临时目录、装包、运行冒烟断言。
param(
    [string]$Configuration = 'Release'
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

# 一定先构建 SDK：包里必须带当前版本的库，否则消费方会在能力位核对时直接拒绝。
Write-Host '== 构建 SDK'
# 不靠 build-sdk 的退出码（在管道/嵌套调用里不可靠），改为核对它产出的 zip。
$log = Join-Path $root 'dist/build-sdk.log'
New-Item -ItemType Directory -Force -Path (Join-Path $root 'dist') | Out-Null
& (Join-Path $root 'tools/build-sdk.ps1') *> $log
$version = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$zipPath = Get-ChildItem (Join-Path $root 'dist') -Filter "scratchmark-$version-*.zip" |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $zipPath) { Get-Content $log | Select-Object -Last 15; throw "build-sdk 没有产出 scratchmark-$version-*.zip" }
$zipPath = $zipPath.FullName
$unpacked = Join-Path $env:TEMP ('scratchmark-nuget-sdk-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
Expand-Archive -LiteralPath $zipPath -DestinationPath $unpacked
$dll = Get-ChildItem $unpacked -Recurse -Filter 'scratchmark.dll' | Where-Object { $_.FullName -match '\\bin\\' } |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $dll) { throw '找不到 scratchmark.dll' }
Write-Host "原生库: $($dll.FullName)"

$packDir = Join-Path $root 'dist/nuget'
New-Item -ItemType Directory -Force -Path $packDir | Out-Null
& dotnet pack (Join-Path $root 'bindings/csharp/Scratchmark/Scratchmark.csproj') `
    -c $Configuration -o $packDir -p:ScratchmarkNativeDll=$($dll.FullName)
if ($LASTEXITCODE -ne 0) { throw 'dotnet pack 失败' }

$nupkg = Get-ChildItem $packDir -Filter 'Scratchmark.$version.nupkg' | Select-Object -First 1
if (-not $nupkg) { throw '没有产出 nupkg' }
Write-Host ("包: {0} ({1:N0} 字节)" -f $nupkg.Name, $nupkg.Length)

# 验证：临时工程装这个包，用无参 Engine() 跑一段冒烟。
$probe = Join-Path $env:TEMP ('scratchmark-nuget-' + [guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force -Path $probe | Out-Null
Copy-Item (Join-Path $root 'tools/nuget-probe/Program.cs') $probe -Force
Copy-Item (Join-Path $root 'tools/nuget-probe/Probe.csproj') $probe -Force
# 同名同版本包会被全局缓存复用，清掉再装，否则验证的是上一次的包。
$cachePath = Join-Path $env:USERPROFILE ".nuget/packages/scratchmark/$version"
if (Test-Path $cachePath) { Remove-Item $cachePath -Recurse -Force }

Push-Location $probe
try {
    & dotnet add package Scratchmark --source $packDir --version $version 2>&1 | Select-Object -Last 3
    if ($LASTEXITCODE -ne 0) { throw '安装本地包失败' }
    & dotnet run -c Release
    if ($LASTEXITCODE -ne 0) { throw 'NuGet 消费方冒烟失败' }
}
finally {
    Pop-Location
    Remove-Item $probe -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Host "NuGet 验收通过: $($nupkg.FullName)"
