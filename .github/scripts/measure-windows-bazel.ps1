<#
Collect lightweight host telemetry around Windows Bazel CI runs.

The monitor mode intentionally uses built-in Windows performance counters so
the workflow does not need to install a profiler on the runner. The summarize
mode keeps the useful aggregate in the job log while the raw CSV remains
available as an artifact for deeper analysis.
#>

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('monitor', 'summarize')]
  [string]$Mode,

  [Parameter(Mandatory = $true)]
  [string]$OutputPath,

  [string]$StopFile,

  [ValidateRange(1, 60)]
  [int]$IntervalSeconds = 15
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-SingleCounterValue {
  param(
    [Parameter(Mandatory = $true)]
    [object]$CounterSample,

    [Parameter(Mandatory = $true)]
    [string]$PathPattern
  )

  $sample = $CounterSample.CounterSamples |
    Where-Object { $_.Path -like $PathPattern } |
    Select-Object -First 1
  if ($null -eq $sample) {
    return 0
  }
  return [double]$sample.CookedValue
}

function Get-SummedCounterValue {
  param(
    [Parameter(Mandatory = $true)]
    [object]$CounterSample,

    [Parameter(Mandatory = $true)]
    [string]$PathPattern
  )

  $samples = $CounterSample.CounterSamples |
    Where-Object { $_.Path -like $PathPattern }
  if ($null -eq $samples) {
    return 0
  }
  return [double](($samples | Measure-Object -Property CookedValue -Sum).Sum)
}

function Get-Percentile {
  param(
    [Parameter(Mandatory = $true)]
    [double[]]$Values,

    [Parameter(Mandatory = $true)]
    [ValidateRange(0, 100)]
    [int]$Percentile
  )

  if ($Values.Count -eq 0) {
    return 0
  }
  $sorted = $Values | Sort-Object
  $index = [math]::Ceiling(($Percentile / 100) * $sorted.Count) - 1
  return [double]$sorted[[math]::Max(0, $index)]
}

function Get-NumericColumn {
  param(
    [Parameter(Mandatory = $true)]
    [object[]]$Rows,

    [Parameter(Mandatory = $true)]
    [string]$Name
  )

  return [double[]]@($Rows | ForEach-Object { [double]$_.$Name })
}

function Format-BytesPerSecond {
  param([double]$Value)

  return '{0:N1} MiB/s' -f ($Value / 1MB)
}

function Write-Summary {
  if (-not (Test-Path -LiteralPath $OutputPath)) {
    Write-Output "No Windows Bazel telemetry file found at $OutputPath."
    return
  }

  $rows = @(Import-Csv -LiteralPath $OutputPath)
  if ($rows.Count -eq 0) {
    Write-Output "Windows Bazel telemetry file at $OutputPath had no samples."
    return
  }

  $cpu = Get-NumericColumn -Rows $rows -Name cpu_percent
  $processorQueue = Get-NumericColumn -Rows $rows -Name processor_queue_length
  $diskRead = Get-NumericColumn -Rows $rows -Name disk_read_bytes_per_sec
  $diskWrite = Get-NumericColumn -Rows $rows -Name disk_write_bytes_per_sec
  $diskLatency = Get-NumericColumn -Rows $rows -Name disk_seconds_per_transfer
  $diskQueue = Get-NumericColumn -Rows $rows -Name disk_queue_length
  $networkReceived = Get-NumericColumn -Rows $rows -Name network_received_bytes_per_sec
  $networkSent = Get-NumericColumn -Rows $rows -Name network_sent_bytes_per_sec
  $memory = Get-NumericColumn -Rows $rows -Name available_memory_mib

  $sampledSeconds = $rows.Count * $IntervalSeconds
  $networkBytes = (($networkReceived | Measure-Object -Average).Average + ($networkSent | Measure-Object -Average).Average) * $sampledSeconds
  $diskBytes = (($diskRead | Measure-Object -Average).Average + ($diskWrite | Measure-Object -Average).Average) * $sampledSeconds

  Write-Output 'Windows Bazel host telemetry summary:'
  Write-Output ('  samples: {0} (~{1:N0}s)' -f $rows.Count, $sampledSeconds)
  Write-Output ('  cpu: avg {0:N1}%, p95 {1:N1}%, max {2:N1}%; processor queue max {3:N1}' -f
    ($cpu | Measure-Object -Average).Average,
    (Get-Percentile -Values $cpu -Percentile 95),
    ($cpu | Measure-Object -Maximum).Maximum,
    ($processorQueue | Measure-Object -Maximum).Maximum)
  Write-Output ('  disk: read avg {0}, write avg {1}, p95 latency {2:N1}ms, queue max {3:N1}, sampled total {4:N2} GiB' -f
    (Format-BytesPerSecond (($diskRead | Measure-Object -Average).Average)),
    (Format-BytesPerSecond (($diskWrite | Measure-Object -Average).Average)),
    ((Get-Percentile -Values $diskLatency -Percentile 95) * 1000),
    ($diskQueue | Measure-Object -Maximum).Maximum,
    ($diskBytes / 1GB))
  Write-Output ('  network: receive avg {0}, send avg {1}, sampled total {2:N2} GiB' -f
    (Format-BytesPerSecond (($networkReceived | Measure-Object -Average).Average)),
    (Format-BytesPerSecond (($networkSent | Measure-Object -Average).Average)),
    ($networkBytes / 1GB))
  Write-Output ('  memory: minimum {0:N0} MiB available' -f ($memory | Measure-Object -Minimum).Minimum)
}

if ($Mode -eq 'summarize') {
  Write-Summary
  exit 0
}

if ([string]::IsNullOrWhiteSpace($StopFile)) {
  throw 'monitor mode requires -StopFile.'
}

$outputDirectory = Split-Path -Parent $OutputPath
if (-not [string]::IsNullOrWhiteSpace($outputDirectory)) {
  New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
}

$counterPaths = @(
  '\Processor(_Total)\% Processor Time',
  '\System\Processor Queue Length',
  '\PhysicalDisk(_Total)\Disk Read Bytes/sec',
  '\PhysicalDisk(_Total)\Disk Write Bytes/sec',
  '\PhysicalDisk(_Total)\Avg. Disk sec/Transfer',
  '\PhysicalDisk(_Total)\Avg. Disk Queue Length',
  '\Network Interface(*)\Bytes Received/sec',
  '\Network Interface(*)\Bytes Sent/sec',
  '\Memory\Available MBytes'
)

while (-not (Test-Path -LiteralPath $StopFile)) {
  try {
    $sample = Get-Counter -Counter $counterPaths -ErrorAction Stop
    $row = [pscustomobject]@{
      timestamp = (Get-Date).ToUniversalTime().ToString('o')
      cpu_percent = Get-SingleCounterValue -CounterSample $sample -PathPattern '*\processor(_total)\% processor time'
      processor_queue_length = Get-SingleCounterValue -CounterSample $sample -PathPattern '*\system\processor queue length'
      disk_read_bytes_per_sec = Get-SingleCounterValue -CounterSample $sample -PathPattern '*\physicaldisk(_total)\disk read bytes/sec'
      disk_write_bytes_per_sec = Get-SingleCounterValue -CounterSample $sample -PathPattern '*\physicaldisk(_total)\disk write bytes/sec'
      disk_seconds_per_transfer = Get-SingleCounterValue -CounterSample $sample -PathPattern '*\physicaldisk(_total)\avg. disk sec/transfer'
      disk_queue_length = Get-SingleCounterValue -CounterSample $sample -PathPattern '*\physicaldisk(_total)\avg. disk queue length'
      network_received_bytes_per_sec = Get-SummedCounterValue -CounterSample $sample -PathPattern '*\network interface(*)\bytes received/sec'
      network_sent_bytes_per_sec = Get-SummedCounterValue -CounterSample $sample -PathPattern '*\network interface(*)\bytes sent/sec'
      available_memory_mib = Get-SingleCounterValue -CounterSample $sample -PathPattern '*\memory\available mbytes'
    }
    $row | Export-Csv -LiteralPath $OutputPath -Append -NoTypeInformation
  }
  catch {
    Write-Warning "Windows Bazel telemetry sample failed: $($_.Exception.Message)"
  }

  # Check the stop file once a second so the monitor does not add up to one
  # full sampling interval to the measured workflow step during cleanup.
  for ($second = 0; $second -lt $IntervalSeconds; $second += 1) {
    if (Test-Path -LiteralPath $StopFile) {
      break
    }
    Start-Sleep -Seconds 1
  }
}
