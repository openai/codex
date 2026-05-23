param(
    [Parameter(Mandatory = $true)]
    [string]$Target,

    [string]$HostArch = ""
)

# Cargo can cross-compile the Rust code for Windows ARM64 on a Windows x64
# runner, but rustup alone does not expose the matching MSVC/UCRT include and
# library paths. Ask Visual Studio for the target-specific developer
# environment, then persist the relevant variables through GITHUB_ENV so the
# later Cargo step sees the same environment as a normal VsDevCmd shell.
switch ($Target) {
    "x86_64-pc-windows-msvc" {
        $TargetArch = "x64"
        $RequiredComponent = "Microsoft.VisualStudio.Component.VC.Tools.x86.x64"
    }
    "aarch64-pc-windows-msvc" {
        $TargetArch = "arm64"
        $RequiredComponent = "Microsoft.VisualStudio.Component.VC.Tools.ARM64"
    }
    default {
        throw "Unsupported Windows MSVC target: $Target"
    }
}

# VsDevCmd needs both sides of the cross compile: the architecture of the
# machine running the tools and the architecture of the binaries being linked.
# Infer the host from the runner unless a caller needs to override it.
if (-not $HostArch) {
    $HostArch = if ($env:PROCESSOR_ARCHITEW6432 -eq "ARM64" -or $env:PROCESSOR_ARCHITECTURE -eq "ARM64") {
        "arm64"
    } else {
        "x64"
    }
}

$VsWhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path $VsWhere)) {
    throw "vswhere.exe not found"
}

# Require the target VC tools component, not merely any Visual Studio install,
# so an x64 archive producer cannot silently link ARM64 tests with the wrong
# SDK/toolchain layout.
$InstallPath = & $VsWhere -latest -products * -requires $RequiredComponent -property installationPath 2>$null
if (-not $InstallPath) {
    throw "Could not locate a Visual Studio installation with component $RequiredComponent"
}

$VsDevCmd = Join-Path $InstallPath "Common7\Tools\VsDevCmd.bat"
if (-not (Test-Path $VsDevCmd)) {
    throw "VsDevCmd.bat not found at $VsDevCmd"
}

$VarsToExport = @(
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "PATH",
    "UCRTVersion",
    "UniversalCRTSdkDir",
    "VCINSTALLDIR",
    "VCToolsInstallDir",
    "WindowsLibPath",
    "WindowsSdkBinPath",
    "WindowsSdkDir",
    "WindowsSDKLibVersion",
    "WindowsSDKVersion"
)

# Run VsDevCmd inside cmd.exe because it is a batch file, then copy just the
# variables Cargo/rustc need into the GitHub Actions environment file. PowerShell
# cannot mutate the parent composite-action environment directly.
$EnvLines = & cmd.exe /c ('"{0}" -no_logo -arch={1} -host_arch={2} >nul && set' -f $VsDevCmd, $TargetArch, $HostArch)
$VcToolsInstallDir = $null
foreach ($Line in $EnvLines) {
    if ($Line -notmatch "^(.*?)=(.*)$") {
        continue
    }

    $Name = $Matches[1]
    $Value = $Matches[2]
    if ($VarsToExport -contains $Name) {
        if ($Name -ieq "Path") {
            $Name = "PATH"
        }
        if ($Name -eq "VCToolsInstallDir") {
            $VcToolsInstallDir = $Value
        }
        "$Name=$Value" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
    }
}

if (-not $VcToolsInstallDir) {
    throw "VCToolsInstallDir was not exported by VsDevCmd.bat"
}

# Prefer Rust's bundled linker when rustup provides one, then Visual Studio's
# LLVM linker, and finally MSVC link.exe. This keeps the cross-compile path close
# to Rust's normal Windows MSVC behavior while still working on runner images
# where one of those linkers is absent.
$Linker = $null
$Rustc = Get-Command rustc -ErrorAction SilentlyContinue
if ($Rustc) {
    $Sysroot = (& rustc --print sysroot 2>$null).Trim()
    $RustHost = & rustc -vV 2>$null | Select-String "^host: " | ForEach-Object { $_.Line.Substring(6) }
    if ($RustHost) {
        $RustHost = $RustHost.Trim()
    }
    if ($Sysroot -and $RustHost) {
        $RustLld = Join-Path $Sysroot "lib\rustlib\$RustHost\bin\rust-lld.exe"
        if (Test-Path $RustLld) {
            $Linker = $RustLld
        }
    }
}
if (-not $Linker) {
    $Linker = Join-Path $InstallPath "VC\Tools\Llvm\x64\bin\lld-link.exe"
}
if (-not (Test-Path $Linker)) {
    $Linker = Join-Path $VcToolsInstallDir "bin\Host${HostArch}\${TargetArch}\link.exe"
}
if (-not (Test-Path $Linker)) {
    throw "Windows linker not found at $Linker"
}

# rustc passes `/arm64hazardfree` for ARM64 MSVC links. The lld variants on our
# Windows x64 archive producers reject that flag, including when rustc places it
# inside a response file. Compile a tiny forwarding wrapper that strips only
# that unsupported flag, then delegate every other argument to the real linker.
if ($TargetArch -eq "arm64" -and (Split-Path -Leaf $Linker) -match "lld") {
    $WrapperDir = Join-Path $env:RUNNER_TEMP "msvc-lld-wrapper"
    New-Item -Path $WrapperDir -ItemType Directory -Force | Out-Null
    $WrapperPath = Join-Path $WrapperDir "lld-link-wrapper.exe"
    $WrapperSource = @'
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Text;
using System.Text.RegularExpressions;

internal static class Program
{
    private static int Main(string[] args)
    {
        var linker = Environment.GetEnvironmentVariable("MSVC_REAL_LINKER");
        if (string.IsNullOrEmpty(linker))
        {
            Console.Error.WriteLine("MSVC_REAL_LINKER is not set");
            return 1;
        }

        var startInfo = new ProcessStartInfo(linker)
        {
            UseShellExecute = false,
        };
        var filteredArgs = new List<string> { "-flavor", "link", "/defaultlib:ucrt", "/nodefaultlib:libucrt" };
        foreach (var arg in args)
        {
            if (!string.Equals(arg, "/arm64hazardfree", StringComparison.OrdinalIgnoreCase))
            {
                filteredArgs.Add(QuoteArgument(FilterResponseFile(arg)));
            }
        }
        startInfo.Arguments = string.Join(" ", filteredArgs);

        using var process = Process.Start(startInfo);
        if (process is null)
        {
            Console.Error.WriteLine($"Failed to start linker: {linker}");
            return 1;
        }

        process.WaitForExit();
        return process.ExitCode;
    }

    private static string FilterResponseFile(string argument)
    {
        if (argument.Length < 2 || argument[0] != '@')
        {
            return argument;
        }

        var responsePath = argument.Substring(1);
        if (!File.Exists(responsePath))
        {
            return argument;
        }

        var filteredResponsePath = Path.Combine(Path.GetTempPath(), Path.GetRandomFileName() + ".rsp");
        var responseContents = Regex.Replace(
            File.ReadAllText(responsePath),
            "/arm64hazardfree",
            string.Empty,
            RegexOptions.IgnoreCase);
        File.WriteAllText(filteredResponsePath, responseContents);
        return "@" + filteredResponsePath;
    }

    private static string QuoteArgument(string argument)
    {
        if (argument.Length == 0)
        {
            return "\"\"";
        }
        if (argument.IndexOfAny(new[] { ' ', '\t', '"' }) < 0)
        {
            return argument;
        }

        var quoted = new StringBuilder("\"");
        var backslashes = 0;
        foreach (var character in argument)
        {
            if (character == '\\')
            {
                backslashes++;
                continue;
            }
            if (character == '"')
            {
                quoted.Append('\\', (backslashes * 2) + 1);
                quoted.Append(character);
                backslashes = 0;
                continue;
            }

            quoted.Append('\\', backslashes);
            backslashes = 0;
            quoted.Append(character);
        }
        quoted.Append('\\', backslashes * 2);
        quoted.Append('"');
        return quoted.ToString();
    }
}
'@
    $WrapperSourcePath = Join-Path $WrapperDir "lld-link-wrapper.cs"
    $WrapperSource | Out-File -FilePath $WrapperSourcePath -Encoding utf8
    $Csc = Join-Path $InstallPath "MSBuild\Current\Bin\Roslyn\csc.exe"
    if (-not (Test-Path $Csc)) {
        throw "csc.exe not found at $Csc"
    }
    & $Csc /nologo /target:exe /out:$WrapperPath $WrapperSourcePath
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to compile lld-link wrapper"
    }
    "MSVC_REAL_LINKER=$Linker" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
    $Linker = $WrapperPath
}

Write-Output "Using Windows linker: $Linker"
$CargoTarget = $Target.ToUpperInvariant().Replace("-", "_")
"CARGO_TARGET_${CargoTarget}_LINKER=$Linker" | Out-File -FilePath $env:GITHUB_ENV -Encoding utf8 -Append
