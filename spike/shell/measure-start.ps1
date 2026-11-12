# Window probe: cold start and memory of the built RequestMaster Spike (stage 0 spike).
#
# Runs the release exe N times. Each run: delete the ready file, start the exe, wait for the ready
# file (the app writes it after the first paint), read the in-app ms, wait SettleMs, sum WorkingSet64
# of the app and of all msedgewebview2.exe processes descending from it, close the app.
# Prints median and max of the start time and of the memory.
#
# Usage: powershell -ExecutionPolicy Bypass -File spike\shell\measure-start.ps1 [-Runs 10]

param(
    [int]$Runs = 10,
    [int]$TimeoutSec = 30,
    [int]$SettleMs = 1000
)

$ErrorActionPreference = 'Stop'

$exe = Join-Path $PSScriptRoot 'src-tauri\target\release\requestmaster-spike.exe'
$readyFile = Join-Path $env:TEMP 'requestmaster-spike-ready.txt'

if (-not (Test-Path $exe)) {
    Write-Error "Exe not found: $exe. Build first: cd spike\shell; npm run tauri build"
}
if (Get-Process -Name 'requestmaster-spike' -ErrorAction SilentlyContinue) {
    Write-Error 'RequestMaster Spike is already running. Close it and run the script again.'
}

# The app process plus every msedgewebview2.exe whose parent chain leads to it.
function Get-TreeIds([int]$rootId) {
    $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name
    $children = @{}
    foreach ($p in $all) {
        $key = [int]$p.ParentProcessId
        if (-not $children.ContainsKey($key)) { $children[$key] = @() }
        $children[$key] += $p
    }
    $ids = @($rootId)
    $queue = New-Object System.Collections.Queue
    $queue.Enqueue($rootId)
    while ($queue.Count -gt 0) {
        $id = $queue.Dequeue()
        if (-not $children.ContainsKey($id)) { continue }
        foreach ($c in $children[$id]) {
            if ($c.Name -ieq 'msedgewebview2.exe') {
                $ids += [int]$c.ProcessId
                $queue.Enqueue([int]$c.ProcessId)
            }
        }
    }
    return $ids
}

function Get-Median([double[]]$values) {
    $sorted = $values | Sort-Object
    $n = $sorted.Count
    if ($n % 2 -eq 1) { return $sorted[($n - 1) / 2] }
    return ($sorted[$n / 2 - 1] + $sorted[$n / 2]) / 2
}

$startMs = @()
$wallMs = @()
$memMb = @()

for ($i = 1; $i -le $Runs; $i++) {
    Remove-Item $readyFile -ErrorAction SilentlyContinue

    $sw = [Diagnostics.Stopwatch]::StartNew()
    $proc = Start-Process -FilePath $exe -PassThru
    while (-not (Test-Path $readyFile)) {
        if ($sw.Elapsed.TotalSeconds -gt $TimeoutSec) {
            Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
            Write-Error "Run ${i}: no ready file after $TimeoutSec s"
        }
        Start-Sleep -Milliseconds 10
    }
    $wall = $sw.ElapsedMilliseconds

    # The file may be visible a moment before its content is flushed.
    $text = ''
    while (-not $text) {
        $text = (Get-Content $readyFile -Raw -ErrorAction SilentlyContinue)
        if (-not $text) { Start-Sleep -Milliseconds 10 }
    }
    $ms = [int]$text.Trim()

    Start-Sleep -Milliseconds $SettleMs
    $ids = Get-TreeIds $proc.Id
    $bytes = 0
    foreach ($id in $ids) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($p) { $bytes += $p.WorkingSet64 }
    }
    $mb = [math]::Round($bytes / 1MB, 1)

    $startMs += $ms
    $wallMs += $wall
    $memMb += $mb
    Write-Host ("run {0,2}: ready {1,5} ms (wall {2,5} ms), memory {3,7} MB in {4} processes" -f $i, $ms, $wall, $mb, $ids.Count)

    # Close gracefully, then make sure the whole tree is gone before the next run.
    $null = $proc.CloseMainWindow()
    if (-not $proc.WaitForExit(5000)) { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
    $deadline = (Get-Date).AddSeconds(10)
    while ((Get-Date) -lt $deadline) {
        $alive = $ids | Where-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }
        if (-not $alive) { break }
        Start-Sleep -Milliseconds 100
    }
}

Write-Host ''
Write-Host ("start (in-app, ms since main): median {0}, max {1}" -f (Get-Median $startMs), ($startMs | Measure-Object -Maximum).Maximum)
Write-Host ("start (wall, launch -> ready file): median {0}, max {1}" -f (Get-Median $wallMs), ($wallMs | Measure-Object -Maximum).Maximum)
Write-Host ("memory (app + its WebView2, MB): median {0}, max {1}" -f (Get-Median $memMb), ($memMb | Measure-Object -Maximum).Maximum)
