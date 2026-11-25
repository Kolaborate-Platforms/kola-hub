[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [Parameter(Mandatory = $true)][string]$Csv,
    [ValidateRange(1, 100)][int]$Runs = 5,
    [ValidateRange(1, 3600)][int]$IdleSeconds = 10
)

$ErrorActionPreference = 'Stop'
$TreeScanMs = 500
$exePath = (Resolve-Path -LiteralPath $Exe).Path
$csvPath = (Resolve-Path -LiteralPath $Csv).Path
if (-not (Test-Path -LiteralPath $exePath -PathType Leaf)) { throw "Exe not found: $Exe" }
if (-not (Test-Path -LiteralPath $csvPath -PathType Leaf)) { throw "CSV not found: $Csv" }

function Get-ProcessTreeIds([int]$RootId) {
    $all = @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId)
    $ids = New-Object 'System.Collections.Generic.List[int]'
    [void]$ids.Add($RootId)
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($proc in $all) {
            $parent = [int]$proc.ParentProcessId
            if ($ids.Contains($parent) -and -not $ids.Contains([int]$proc.ProcessId)) {
                [void]$ids.Add([int]$proc.ProcessId)
                $changed = $true
            }
        }
    }
    return ,([int[]]$ids.ToArray())
}

function Get-TreeWorkingSet([int[]]$Ids) {
    $sum = [int64]0
    foreach ($id in $Ids) {
        try { $sum += [int64](Get-Process -Id $id -ErrorAction Stop).WorkingSet64 } catch { }
    }
    return $sum
}

function Stop-ProcessTree([int]$RootId) {
    $ids = Get-ProcessTreeIds $RootId
    [array]::Reverse($ids)
    foreach ($id in $ids) {
        try { Stop-Process -Id $id -Force -ErrorAction Stop } catch { }
    }
}

function Start-App([string]$BenchPath) {
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $exePath
    $start.WorkingDirectory = Split-Path -Parent $exePath
    $start.UseShellExecute = $false
    $start.CreateNoWindow = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.EnvironmentVariables['KH_CSV'] = $csvPath
    if ($BenchPath) { $start.EnvironmentVariables['KH_BENCH_OUT'] = $BenchPath }
    else { $start.EnvironmentVariables.Remove('KH_BENCH_OUT') }
    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $start
    [void]$p.Start()
    # Drain both redirected streams asynchronously so a verbose app cannot block.
    $null = $p.StandardOutput.ReadToEndAsync()
    $null = $p.StandardError.ReadToEndAsync()
    return $p
}

function Read-BenchLines([string]$Path, [ref]$Offset) {
    $found = @()
    if (Test-Path -LiteralPath $Path) {
        $stream = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
        try {
            [void]$stream.Seek($Offset.Value, [System.IO.SeekOrigin]::Begin)
            $reader = New-Object System.IO.StreamReader($stream)
            $text = $reader.ReadToEnd()
            $reader.Dispose()
        } finally { $stream.Dispose() }
        $lastNewline = $text.LastIndexOf("`n")
        if ($lastNewline -ge 0) {
            $complete = $text.Substring(0, $lastNewline + 1)
            $Offset.Value += [Text.Encoding]::UTF8.GetByteCount($complete)
            foreach ($line in ($complete -split "`r?`n")) {
                if ($line.Trim()) { $found += $line.Trim() }
            }
        }
    }
    return ,$found
}

function Format-MB([double]$Bytes) { return ('{0:N1}' -f ($Bytes / 1MB)) }
function Get-Median([double[]]$Values) {
    $sorted = @($Values | Sort-Object)
    $n = $sorted.Count
    if ($n % 2) { return [double]$sorted[[int][math]::Floor($n / 2)] }
    return ([double]$sorted[$n / 2 - 1] + [double]$sorted[$n / 2]) / 2
}

$oldCsv = [Environment]::GetEnvironmentVariable('KH_CSV', 'Process')
$oldBench = [Environment]::GetEnvironmentVariable('KH_BENCH_OUT', 'Process')
$benchRuns = @()
$idleRuns = @()
$allJson = New-Object 'System.Collections.Generic.List[string]'
try {
    [Environment]::SetEnvironmentVariable('KH_CSV', $csvPath, 'Process')
    for ($run = 1; $run -le $Runs; $run++) {
        $benchFile = Join-Path ([IO.Path]::GetTempPath()) ("kh-bench-{0}.jsonl" -f [guid]::NewGuid().ToString('N'))
        $proc = $null
        try {
            $timer = [Diagnostics.Stopwatch]::StartNew()
            $proc = Start-App $benchFile
            $peak = [int64]0
            $readyMs = $null
            $offset = [int64]0
            $deadline = [datetime]::UtcNow.AddSeconds(60)
            # Listing processes is slow on old machines, so it runs less often than the ready check.
            $treeIds = @($proc.Id)
            $nextScan = [datetime]::UtcNow
            while ([datetime]::UtcNow -lt $deadline) {
                foreach ($line in (Read-BenchLines $benchFile ([ref]$offset))) {
                    $allJson.Add($line)
                    try { $event = $line | ConvertFrom-Json } catch { throw "Invalid JSON line in bench output: $line" }
                    if ($event.event -eq 'error') { throw "Bench run $run reported error: $($event.message)" }
                    if ($event.event -eq 'ready' -and $null -eq $readyMs) { $readyMs = $timer.Elapsed.TotalMilliseconds }
                }
                if ($proc.HasExited) { break }
                if ([datetime]::UtcNow -ge $nextScan) {
                    $treeIds = Get-ProcessTreeIds $proc.Id
                    $nextScan = [datetime]::UtcNow.AddMilliseconds($TreeScanMs)
                }
                $peak = [math]::Max($peak, (Get-TreeWorkingSet $treeIds))
                Start-Sleep -Milliseconds 50
            }
            if (-not $proc.HasExited) { Stop-ProcessTree $proc.Id; throw "Bench run $run timed out after 60 seconds" }
            foreach ($line in (Read-BenchLines $benchFile ([ref]$offset))) {
                $allJson.Add($line)
                try { $event = $line | ConvertFrom-Json } catch { throw "Invalid JSON line in bench output: $line" }
                if ($event.event -eq 'error') { throw "Bench run $run reported error: $($event.message)" }
                if ($event.event -eq 'ready' -and $null -eq $readyMs) { $readyMs = $timer.Elapsed.TotalMilliseconds }
            }
            $timer.Stop()
            if ($proc.ExitCode -ne 0) { throw "Bench run $run exited with code $($proc.ExitCode)" }
            if ($null -eq $readyMs) { throw "Bench run $run exited without a ready line" }
            $benchRuns += [pscustomobject]@{ Run = $run; ReadyMs = [double]$readyMs; PeakBytes = [int64]$peak }
        } finally {
            if ($proc -and -not $proc.HasExited) { Stop-ProcessTree $proc.Id }
            Remove-Item -LiteralPath $benchFile -Force -ErrorAction SilentlyContinue
        }

        $proc = $null
        try {
            $proc = Start-App $null
            $idlePeak = [int64]0
            $idleUntil = [datetime]::UtcNow.AddSeconds($IdleSeconds)
            while ([datetime]::UtcNow -lt $idleUntil) {
                if ($proc.HasExited) { throw "Idle run $run exited before $IdleSeconds seconds" }
                $idlePeak = [math]::Max($idlePeak, (Get-TreeWorkingSet (Get-ProcessTreeIds $proc.Id)))
                Start-Sleep -Milliseconds 50
            }
            $idlePeak = [math]::Max($idlePeak, (Get-TreeWorkingSet (Get-ProcessTreeIds $proc.Id)))
            $idleRuns += [pscustomobject]@{ Run = $run; PeakBytes = [int64]$idlePeak }
        } finally {
            if ($proc -and -not $proc.HasExited) { Stop-ProcessTree $proc.Id }
        }
    }
} finally {
    [Environment]::SetEnvironmentVariable('KH_CSV', $oldCsv, 'Process')
    [Environment]::SetEnvironmentVariable('KH_BENCH_OUT', $oldBench, 'Process')
}

$os = Get-CimInstance Win32_OperatingSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$computer = Get-CimInstance Win32_ComputerSystem
Write-Output ("Exe size: {0:N2} MB" -f ((Get-Item -LiteralPath $exePath).Length / 1MB))
Write-Output ("OS: {0} (version {1}, build {2})" -f $os.Caption, $os.Version, $os.BuildNumber)
Write-Output ("CPU: {0}" -f $cpu.Name.Trim())
Write-Output ("Cores: {0} physical, {1} logical" -f $cpu.NumberOfCores, $cpu.NumberOfLogicalProcessors)
Write-Output ("RAM: {0:N1} GB" -f ($computer.TotalPhysicalMemory / 1GB))
Write-Output ''
Write-Output 'Bench JSON lines:'
foreach ($line in $allJson) { Write-Output $line }
Write-Output ''
Write-Output '| Measure | Median | Worst run |'
Write-Output '|---|---:|---:|'
$readyValues = @($benchRuns | ForEach-Object { $_.ReadyMs })
$benchMem = @($benchRuns | ForEach-Object { $_.PeakBytes })
$idleMem = @($idleRuns | ForEach-Object { $_.PeakBytes })
$worstReady = ($benchRuns | Sort-Object ReadyMs -Descending | Select-Object -First 1)
$worstBenchMem = ($benchRuns | Sort-Object PeakBytes -Descending | Select-Object -First 1)
$worstIdleMem = ($idleRuns | Sort-Object PeakBytes -Descending | Select-Object -First 1)
Write-Output ("| Bench launch to ready | {0:N1} ms | {1:N1} ms (run {2}) |" -f (Get-Median $readyValues), $worstReady.ReadyMs, $worstReady.Run)
Write-Output ("| Bench peak process tree | {0} MB | {1} MB (run {2}) |" -f (Format-MB (Get-Median $benchMem)), (Format-MB $worstBenchMem.PeakBytes), $worstBenchMem.Run)
Write-Output ("| Idle peak process tree | {0} MB | {1} MB (run {2}) |" -f (Format-MB (Get-Median $idleMem)), (Format-MB $worstIdleMem.PeakBytes), $worstIdleMem.Run)
