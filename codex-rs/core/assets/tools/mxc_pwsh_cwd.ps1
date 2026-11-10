# PowerShell/PowerShell#27899: a denied ancestor can leave $PWD unset and the
# filesystem drive at its root. Use the process cwd without resolving it again.
# A successful startup or a profile that changed location has already set $PWD.
& {
    if ($ExecutionContext.SessionState.LanguageMode -ne 'FullLanguage') { return }
    if ($null -ne $ExecutionContext.SessionState.PSVariable.GetValue('global:PWD')) { return }
    $drive = $ExecutionContext.SessionState.Drive.Current
    $cwd = [Environment]::CurrentDirectory
    if ($drive.Provider.Name -eq 'FileSystem' -and !$drive.CurrentLocation -and
        $drive.Root -eq [IO.Path]::GetPathRoot($cwd) -and $cwd.Length -gt $drive.Root.Length) {
        $drive.CurrentLocation = $cwd.Substring($drive.Root.Length)
        Microsoft.PowerShell.Utility\Set-Variable -Name PWD -Scope Global -Value $ExecutionContext.SessionState.Path.CurrentLocation -Force
    }
}
