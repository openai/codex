$ErrorActionPreference = 'Stop'
$PSModuleAutoLoadingPreference = 'None'
Import-Module -Name ($PSHOME + '\Modules\Microsoft.PowerShell.Utility\Microsoft.PowerShell.Utility.psd1') -ErrorAction Stop
Import-Module -Name ($PSHOME + '\Modules\Appx\Appx.psd1') -ErrorAction Stop
Import-Module -Name ($PSHOME + '\Modules\Microsoft.PowerShell.Management\Microsoft.PowerShell.Management.psd1') -ErrorAction Stop
if ([Security.Principal.WindowsIdentity]::GetCurrent().User.Value -ne 'S-1-5-18') { throw 'SYSTEM required' }
# Rust sends UTF-8 regardless of the machine's console code page. Keep one reader
# for the complete protocol so read-ahead cannot consume COMMIT or its EOF fence.
$protocolInput = [IO.StreamReader]::new([Console]::OpenStandardInput(), [Text.UTF8Encoding]::new($false, $true), $false)
$planJson = $protocolInput.ReadLine()
if (!$planJson) { return }
$plan = Microsoft.PowerShell.Utility\ConvertFrom-Json -InputObject $planJson
if ([string]::IsNullOrEmpty($plan.record.runtime.retiring)) { throw 'Retirement generation required' }
if (@($plan.targets).Count -gt 2) { throw 'Too many cleanup tokens' }
Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class CleanupNative {
 [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct Profile { public int size,flags; public string user,path,def,server,policy; public IntPtr handle; }
 [StructLayout(LayoutKind.Sequential)] public struct Luid { public uint lo; public int hi; }
 [StructLayout(LayoutKind.Sequential)] public struct Priv { public uint count; public Luid luid; public uint attributes; }
 [DllImport("userenv.dll",CharSet=CharSet.Unicode,SetLastError=true)] public static extern bool LoadUserProfile(IntPtr t,ref Profile p);
 [DllImport("userenv.dll",SetLastError=true)] public static extern bool UnloadUserProfile(IntPtr t,IntPtr p);
 [DllImport("advapi32.dll",SetLastError=true)] public static extern bool ImpersonateLoggedOnUser(IntPtr t);
 [DllImport("advapi32.dll",SetLastError=true)] public static extern bool RevertToSelf();
 [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr h);
 [DllImport("kernel32.dll")] public static extern IntPtr GetCurrentProcess();
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr p,uint a,out IntPtr t);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool LookupPrivilegeValue(string s,string n,out Luid l);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool AdjustTokenPrivileges(IntPtr t,bool d,ref Priv p,int len,IntPtr old,IntPtr ret);
 public static void Enable(string n) { IntPtr t; Luid l; if(!OpenProcessToken(GetCurrentProcess(),0x28,out t))throw new Win32Exception(); try { if(!LookupPrivilegeValue(null,n,out l))throw new Win32Exception(); Priv p=new Priv{count=1,luid=l,attributes=2}; if(!AdjustTokenPrivileges(t,false,ref p,0,IntPtr.Zero,IntPtr.Zero))throw new Win32Exception(); int e=Marshal.GetLastWin32Error(); if(e!=0)throw new Win32Exception(e); } finally { CloseHandle(t); } }
 public static void Revert() { if(!RevertToSelf())Environment.FailFast("Cleanup impersonation revert failed"); }
}
"@
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$null = [Windows.Management.Deployment.PackageManager,Windows.Management.Deployment,ContentType=WindowsRuntime]
$null = [Windows.Management.Deployment.DeploymentResult,Windows.Management.Deployment,ContentType=WindowsRuntime]
$null = [Windows.Management.Deployment.DeploymentProgress,Windows.Management.Deployment,ContentType=WindowsRuntime]
$asTask = [System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object { $_.Name -eq 'AsTask' -and $_.IsGenericMethod -and $_.GetGenericArguments().Count -eq 2 -and $_.GetParameters().Count -eq 1 } | Select-Object -First 1
$await = $asTask.MakeGenericMethod([Windows.Management.Deployment.DeploymentResult],[Windows.Management.Deployment.DeploymentProgress])
$profiles = @()
try {
    [CleanupNative]::Enable('SeBackupPrivilege')
    [CleanupNative]::Enable('SeRestorePrivilege')
    foreach ($target in $plan.targets) {
        $token = [IntPtr][long]$target.handle
        $identity = [Security.Principal.WindowsIdentity]::new($token)
        try { if ($identity.User.Value -cne $target.sid) { throw 'Cleanup token SID mismatch' } }
        finally { $identity.Dispose() }
        $profile = New-Object CleanupNative+Profile
        $profile.size = [Runtime.InteropServices.Marshal]::SizeOf($profile)
        $profile.flags = 1
        $profile.user = $target.username
        if (![CleanupNative]::LoadUserProfile($token, [ref]$profile)) { throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error()) }
        $profiles += @{ token = $token; profile = $profile; sid = $target.sid }
    }
    [Console]::Out.WriteLine('READY')
    # EOF without a successful native-cleanup commit only unloads profiles.
    if ($protocolInput.ReadLine() -cne 'COMMIT') { return }
    # Even after COMMIT, removal must wait until the service releases its own package.
    if ($protocolInput.ReadToEnd().Length -ne 0) { return }
    $finished = $false
    while (!$finished) {
        $mutex = $null; $locked = $false; $key = $null; $legacy = $null
        try {
            $security = [Security.AccessControl.MutexSecurity]::new()
            $security.SetSecurityDescriptorSddlForm('D:P(A;;GA;;;SY)(A;;GA;;;BA)')
            $created = $false
            $mutex = [Threading.Mutex]::new($false, 'Global\CodexSandboxSetup', [ref]$created, $security)
            try { $locked = $mutex.WaitOne(30000) }
            catch [Threading.AbandonedMutexException] { $locked = $true }
            if (!$locked) { throw 'Sandbox setup is still active' }
            $key = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($plan.key, $true)
            if (!$key) { return }
            $record = Microsoft.PowerShell.Utility\ConvertFrom-Json -InputObject ([string]$key.GetValue($plan.value))
            if ($record.runtime.retiring -ne $plan.record.runtime.retiring -or
                $record.user_sid -ne $plan.record.user_sid -or
                $record.codex_home -ne $plan.record.codex_home -or
                $record.runtime.package_family -ne $plan.record.runtime.package_family) { return }
            foreach ($entry in $profiles) {
                foreach ($package in @(Appx\Get-AppxPackage -User $entry.sid -ErrorAction Stop)) {
                    if ($package.PackageFamilyName -ne $record.runtime.package_family) { continue }
                    if (![CleanupNative]::ImpersonateLoggedOnUser($entry.token)) { throw 'Cleanup impersonation failed' }
                    try {
                        if ([Security.Principal.WindowsIdentity]::GetCurrent().User.Value -cne $entry.sid) { throw 'Cleanup identity changed' }
                        # Construct and submit synchronously under the retained user, not SYSTEM.
                        $manager = [Windows.Management.Deployment.PackageManager]::new()
                        $operation = $manager.RemovePackageAsync($package.PackageFullName)
                    } finally { [CleanupNative]::Revert() }
                    # Failure to obtain a waiter is not proof the operation stopped.
                    $task = $null
                    while (!$task) {
                        try { $task = $await.Invoke($null, @($operation)) }
                        catch { Microsoft.PowerShell.Utility\Start-Sleep -Seconds 1 }
                    }
                    while (!$task.IsCompleted) { Microsoft.PowerShell.Utility\Start-Sleep -Milliseconds 50 }
                    $result = $task.GetAwaiter().GetResult()
                    if ($result.ExtendedErrorCode -and $result.ExtendedErrorCode.HResult -ne 0) { throw $result.ErrorText }
                }
            }
            foreach ($account in $plan.record.runtime.accounts) {
                foreach ($package in @(Appx\Get-AppxPackage -User $account.user_sid -ErrorAction Stop)) {
                    if ($package.PackageFamilyName -eq $record.runtime.package_family) { throw 'Runtime registration remains' }
                }
            }
            $legacy = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey($plan.legacy_key, $true)
            if (!$legacy) { throw 'Installation parent is missing' }
            $legacyJson = $legacy.GetValue($plan.value)
            if ($legacyJson) {
                $owner = Microsoft.PowerShell.Utility\ConvertFrom-Json -InputObject ([string]$legacyJson)
                if ($owner.user_sid -eq $plan.record.user_sid -and $owner.codex_home -eq $plan.record.codex_home) { $legacy.DeleteValue($plan.value, $false) }
            }
            $key.Dispose(); $key = $null
            $legacy.Flush()
            [Microsoft.Win32.Registry]::LocalMachine.DeleteSubKey($plan.key, $false)
            $finished = $true
        } catch { [Console]::Error.WriteLine("Runtime package removal deferred: $_") }
        finally {
            if ($key) { $key.Dispose() }
            if ($legacy) { $legacy.Dispose() }
            if ($locked) { $mutex.ReleaseMutex() }
            if ($mutex) { $mutex.Dispose() }
        }
        if (!$finished) { Microsoft.PowerShell.Utility\Start-Sleep -Seconds 30 }
    }
} finally {
    foreach ($entry in $profiles) {
        while (![CleanupNative]::UnloadUserProfile($entry.token, $entry.profile.handle)) {
            Microsoft.PowerShell.Utility\Start-Sleep -Seconds 1
        }
    }
    foreach ($target in $plan.targets) { $null = [CleanupNative]::CloseHandle([IntPtr][long]$target.handle) }
}
# A reinstall may have tried to start the service while the retirement fence blocked it.
while ($true) {
    try {
        foreach ($package in @(Appx\Get-AppxPackage -User $plan.record.user_sid -ErrorAction Stop)) {
            if ($package.PackageFamilyName -eq $plan.record.runtime.package_family) {
                Microsoft.PowerShell.Management\Start-Service -Name $plan.service_name -ErrorAction Stop
                break
            }
        }
        break
    } catch { Microsoft.PowerShell.Utility\Start-Sleep -Seconds 30 }
}
