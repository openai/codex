# Configure a fast drive for Windows CI jobs.
#
# GitHub-hosted Windows runners do not always expose a secondary D: volume. When
# they do not, try to create a Dev Drive VHD and fall back to C: if the runner
# image does not allow that provisioning path.

function Use-FallbackDrive {
    param([string]$Reason)

    Write-Warning "$Reason Falling back to C:"
    return "C:"
}

function Invoke-BestEffort {
    param([scriptblock]$Script, [string]$Description)

    try {
        & $Script
    } catch {
        Write-Warning "$Description failed: $($_.Exception.Message)"
    }
}

if (Test-Path "D:\") {
    Write-Output "Using existing drive at D:"
    $Drive = "D:"
} else {
    try {
        $VhdPath = Join-Path $env:RUNNER_TEMP "codex-dev-drive.vhdx"
        $SizeBytes = 64GB

        if (Test-Path $VhdPath) {
            Remove-Item -Path $VhdPath -Force
        }

        New-VHD -Path $VhdPath -SizeBytes $SizeBytes -Dynamic -ErrorAction Stop | Out-Null
        $Mounted = Mount-VHD -Path $VhdPath -Passthru -ErrorAction Stop
        $Disk = $Mounted | Get-Disk -ErrorAction Stop
        $Disk | Initialize-Disk -PartitionStyle GPT -ErrorAction Stop
        $Partition = $Disk | New-Partition -AssignDriveLetter -UseMaximumSize -ErrorAction Stop
        $Volume = $Partition | Format-Volume -FileSystem ReFS -NewFileSystemLabel "CodexDevDrive" -DevDrive -Confirm:$false -Force -ErrorAction Stop

        $Drive = "$($Volume.DriveLetter):"

        Invoke-BestEffort { fsutil devdrv trust $Drive } "Trusting Dev Drive $Drive"
        Invoke-BestEffort { fsutil devdrv enable /disallowAv } "Disabling AV filter attachment for Dev Drives"
        Invoke-BestEffort { fsutil devdrv query $Drive } "Querying Dev Drive $Drive"

        Write-Output "Using Dev Drive at $Drive"
    } catch {
        $Drive = Use-FallbackDrive "Failed to create Dev Drive: $($_.Exception.Message)"
    }
}

$Tmp = "$Drive\codex-tmp"
New-Item -Path $Tmp -ItemType Directory -Force | Out-Null

@(
    "DEV_DRIVE=$Drive"
    "TMP=$Tmp"
    "TEMP=$Tmp"
) | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
