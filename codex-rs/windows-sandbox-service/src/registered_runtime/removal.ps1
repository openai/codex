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
 [DllImport("userenv.dll",CharSet=CharSet.Unicode,SetLastError=true)] public static extern bool DeleteProfile(string sid,string path,string computer);
 [StructLayout(LayoutKind.Sequential)] struct User23 { public IntPtr name,fullName,comment; public uint flags; public IntPtr sid; }
 [DllImport("netapi32.dll",CharSet=CharSet.Unicode)] static extern int NetUserGetInfo(string server,string user,int level,out IntPtr info);
 [DllImport("netapi32.dll",CharSet=CharSet.Unicode)] static extern int NetUserDel(string server,string user);
 [DllImport("netapi32.dll")] static extern int NetApiBufferFree(IntPtr info);
 [DllImport("netapi32.dll",CharSet=CharSet.Unicode)] public static extern int NetLocalGroupDel(string server,string group);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode)] static extern IntPtr RegisterEventSource(string server,string source);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode)] static extern bool ReportEvent(IntPtr h,ushort type,ushort category,uint id,IntPtr sid,ushort count,uint size,string[] messages,IntPtr data);
 [DllImport("advapi32.dll")] static extern bool DeregisterEventSource(IntPtr h);
 public static void Log(string source,ushort type,uint id,string message) { IntPtr h=RegisterEventSource(null,source); if(h==IntPtr.Zero)return; try { ReportEvent(h,type,0,id,IntPtr.Zero,1,0,new[]{message.Substring(0,Math.Min(message.Length,4096))},IntPtr.Zero); } finally { DeregisterEventSource(h); } }
 public static void DeleteUser(string user,string sid) { IntPtr p; int status=NetUserGetInfo(null,user,23,out p); if(status==2221)return; if(status!=0)throw new Win32Exception(status); try { User23 info=(User23)Marshal.PtrToStructure(p,typeof(User23)); if(new System.Security.Principal.SecurityIdentifier(info.sid).Value!=sid || (info.flags&2)==0)throw new InvalidOperationException("Sandbox account changed or was re-enabled"); } finally { NetApiBufferFree(p); } status=NetUserDel(null,user); if(status!=0 && status!=2221)throw new Win32Exception(status); }
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
$removedRegistrations = @{}
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
                if ($removedRegistrations.ContainsKey($entry.sid)) { continue }
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
                if ($removedRegistrations.ContainsKey($account.user_sid)) { continue }
                foreach ($package in @(Appx\Get-AppxPackage -User $account.user_sid -ErrorAction Stop)) {
                    if ($package.PackageFamilyName -eq $record.runtime.package_family) { throw 'Runtime registration remains' }
                }
                $removedRegistrations[$account.user_sid] = $true
            }
            foreach ($account in $plan.record.runtime.accounts) {
                $sid = $account.user_sid
                foreach ($entry in $profiles) {
                    if ($entry.sid -ne $sid -or $entry.profile.handle -eq [IntPtr]::Zero) { continue }
                    if (![CleanupNative]::UnloadUserProfile($entry.token, $entry.profile.handle)) { throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error()) }
                    $entry.profile.handle = [IntPtr]::Zero
                }
                $profileKey = [Microsoft.Win32.Registry]::LocalMachine.OpenSubKey("SOFTWARE\Microsoft\Windows NT\CurrentVersion\ProfileList\$sid")
                if ($profileKey) {
                    $profileKey.Dispose()
                    # PowerShell converts $null to an empty string; DeleteProfile requires native NULL.
                    if (![CleanupNative]::DeleteProfile($sid, [NullString]::Value, [NullString]::Value)) { throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error()) }
                }
                [CleanupNative]::DeleteUser(('CodexSandbox' + $account.account), $sid)
                [CleanupNative]::Log($plan.service_name, 4, 3004, "Removed $($account.account) sandbox profile and account")
            }
            $groupSid = $null
            try { if ($plan.group_sid) { $groupSid = ([Security.Principal.NTAccount]::new('CodexSandboxUsers')).Translate([Security.Principal.SecurityIdentifier]).Value } }
            catch [Security.Principal.IdentityNotMappedException] { }
            if ($groupSid) {
                if ($groupSid -cne $plan.group_sid) { throw 'Sandbox group was replaced' }
                $status = [CleanupNative]::NetLocalGroupDel($null, 'CodexSandboxUsers')
                if ($status -ne 0 -and $status -ne 2220) { throw [ComponentModel.Win32Exception]::new($status) }
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
            if ($legacy.SubKeyCount -eq 0 -and $legacy.ValueCount -eq 0) {
                $legacy.Dispose(); $legacy = $null
                [Microsoft.Win32.Registry]::LocalMachine.DeleteSubKey($plan.legacy_key, $false)
            }
            $finished = $true
            [CleanupNative]::Log($plan.service_name, 4, 3003, 'Sandbox uninstall cleanup finished, including registered runtimes, profiles, and accounts')
        } catch { [CleanupNative]::Log($plan.service_name, 1, 3004, "Runtime cleanup deferred: $_") }
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
        if ($entry.profile.handle -eq [IntPtr]::Zero) { continue }
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
