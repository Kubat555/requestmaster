# Measurements of the built RequestMaster (section 10 of the spec): cold start, and the timings of the window.
#
# The program is started with RQM_MEASURE_DIR=<Dir>: it then keeps everything in that folder (app.db, bodies)
# instead of the working database, and appends what the window measures to <Dir>\timings.tsv, one
# "name<TAB>ms" line per measurement. The line "ready" appears when the window is drawn and its tabs are back.
#
# Three modes:
#   (default)  Starts the exe -Runs times. Each run: delete <Dir>\timings.tsv (app.db and bodies stay: the
#              database holds the tabs to restore), start the exe, wait for the "ready" line, close the window.
#              Prints the wall time from the launch to the line, then its median and maximum.
#   -Keep      Starts the exe once for manual work and leaves it running; timings.tsv is kept and appended to.
#   -Report    Starts nothing: reads <Dir>\timings.tsv and prints the count, the median and the maximum of
#              every name.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\measure-start.ps1 -Dir C:\Temp\rqm-measure [-Runs 10]
#   powershell -ExecutionPolicy Bypass -File scripts\measure-start.ps1 -Dir C:\Temp\rqm-measure -Keep
#   powershell -ExecutionPolicy Bypass -File scripts\measure-start.ps1 -Dir C:\Temp\rqm-measure -Report

param(
    [string]$Exe = (Join-Path (Split-Path $PSScriptRoot -Parent) 'target\release\requestmaster.exe'),
    [Parameter(Mandatory = $true)]
    [string]$Dir,
    [int]$Runs = 10,
    [int]$TimeoutSec = 30,
    [switch]$Keep,
    [switch]$Report
)

$ErrorActionPreference = 'Stop'

if ($Keep -and $Report) {
    Write-Error 'Choose one of -Keep and -Report.'
}

if (-not (Test-Path $Dir)) {
    $null = New-Item -ItemType Directory -Force -Path $Dir
}
# The program gets the full path: its working folder is not the one of this script.
$Dir = (Resolve-Path $Dir).Path
$timingsFile = Join-Path $Dir 'timings.tsv'

# The core writes numbers with a dot whatever the language of Windows is.
$invariant = [Globalization.CultureInfo]::InvariantCulture

function Get-Median([double[]]$values) {
    $sorted = @($values | Sort-Object)
    $n = $sorted.Count
    if ($n % 2 -eq 1) { return $sorted[($n - 1) / 2] }
    return ($sorted[$n / 2 - 1] + $sorted[$n / 2]) / 2
}

# The lines of the timings file as (name, ms); empty while the file is not there or cannot be read yet.
function Read-Timings {
    $lines = Get-Content $timingsFile -ErrorAction SilentlyContinue
    foreach ($line in @($lines)) {
        if (-not $line) { continue }
        $parts = $line -split "`t"
        if ($parts.Count -ne 2) { continue }
        $ms = 0.0
        if (-not [double]::TryParse($parts[1], [Globalization.NumberStyles]::Float, $invariant, [ref]$ms)) { continue }
        [pscustomobject]@{ Name = $parts[0]; Ms = $ms }
    }
}

if ($Report) {
    if (-not (Test-Path $timingsFile)) {
        Write-Error "No timings file: $timingsFile"
    }
    $groups = @(Read-Timings | Group-Object Name | Sort-Object Name)
    if ($groups.Count -eq 0) {
        Write-Host "No timings in $timingsFile"
        return
    }
    Write-Host ("{0,-32} {1,7} {2,10} {3,10}" -f 'name', 'count', 'median', 'max')
    foreach ($group in $groups) {
        $values = [double[]]@($group.Group | ForEach-Object { $_.Ms })
        $max = ($values | Measure-Object -Maximum).Maximum
        Write-Host ("{0,-32} {1,7} {2,10:F1} {3,10:F1}" -f $group.Name, $values.Count, (Get-Median $values), $max)
    }
    return
}

if (-not (Test-Path $Exe)) {
    Write-Error "Exe not found: $Exe. Build first: cd ui; npm run tauri build"
}
if (Get-Process -Name 'requestmaster' -ErrorAction SilentlyContinue) {
    Write-Error 'RequestMaster is already running. Close it and run the script again.'
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

# The started program inherits the variable; this shell gets its own value back afterwards.
$previous = $env:RQM_MEASURE_DIR
$env:RQM_MEASURE_DIR = $Dir
try {
    if ($Keep) {
        $proc = Start-Process -FilePath $Exe -PassThru
        Write-Host "RequestMaster is running (process $($proc.Id)) with RQM_MEASURE_DIR=$Dir"
        Write-Host "Timings are appended to $timingsFile; when done, close the program and run the script with -Report."
        return
    }

    $wallMs = @()
    for ($i = 1; $i -le $Runs; $i++) {
        Remove-Item $timingsFile -ErrorAction SilentlyContinue

        $sw = [Diagnostics.Stopwatch]::StartNew()
        $proc = Start-Process -FilePath $Exe -PassThru
        $ready = $null
        while (-not $ready) {
            $ready = Read-Timings | Where-Object { $_.Name -eq 'ready' } | Select-Object -First 1
            if ($ready) { break }
            if ($sw.Elapsed.TotalSeconds -gt $TimeoutSec) {
                Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
                Write-Error "Run ${i}: no ready line in $timingsFile after $TimeoutSec s"
            }
            Start-Sleep -Milliseconds 10
        }
        $wall = $sw.ElapsedMilliseconds
        $wallMs += $wall
        # The in-window number (performance.now() of the window) is printed for reference only.
        Write-Host ("run {0,2}: wall {1,5} ms (in-window ready {2:F0} ms)" -f $i, $wall, $ready.Ms)

        # Close gracefully, so that the session is written down; then make sure the whole tree is gone
        # before the next run.
        $ids = Get-TreeIds $proc.Id
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
    Write-Host ("start (wall, launch -> ready line): median {0}, max {1}, runs {2}" -f (Get-Median $wallMs), ($wallMs | Measure-Object -Maximum).Maximum, $wallMs.Count)
}
finally {
    $env:RQM_MEASURE_DIR = $previous
}
