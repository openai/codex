<#
.SYNOPSIS
    Example Codex CLI notifier for Windows toast notifications.

.DESCRIPTION
    Codex CLI invokes the command configured in the `notify` option after each
    agent turn, passing a single JSON argument. This script parses that payload
    and shows a native Windows toast summarizing the prompt and assistant reply.

    Save this file somewhere in your profile directory and configure

        notify = ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "C:/Users/<you>/codex-notify.ps1"]

    in `~/.codex/config.toml` (note forward slashes for TOML strings).
#>
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$NotificationJson
)

# Try to parse the JSON payload Codex passes to `notify` commands.
try {
    $payload = $NotificationJson | ConvertFrom-Json -ErrorAction Stop
} catch {
    Write-Verbose "codex-cli-notify: failed to parse notification payload: $_"
    return
}

if ($payload.type -ne 'agent-turn-complete') {
    Write-Verbose "codex-cli-notify: ignoring payload type '$($payload.type)'"
    return
}

$turnId = $payload.'turn-id'
$inputs = @()
if ($payload.'input-messages' -is [System.Collections.IEnumerable]) {
    $inputs = @($payload.'input-messages' | Where-Object { $_ })
}

$assistantText = $payload.'last-assistant-message'
if ([string]::IsNullOrWhiteSpace($assistantText)) {
    $assistantText = 'Response finished.'
}

$firstPromptLine = $null
if ($inputs.Count -gt 0) {
    $firstPromptLine = ($inputs[0] -split "`r?`n", 2)[0]
}

$toastBody = $assistantText -replace "`r?`n", ' '
if ($toastBody.Length -gt 200) {
    $toastBody = $toastBody.Substring(0, 197) + '...'
}

if (-not [string]::IsNullOrWhiteSpace($firstPromptLine)) {
    $toastBody = "${firstPromptLine}: $toastBody"
    if ($toastBody.Length -gt 200) {
        $toastBody = $toastBody.Substring(0, 197) + '...'
    }
}

try {
    [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
    [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom.XmlDocument, ContentType = WindowsRuntime] | Out-Null

    $template = [Windows.UI.Notifications.ToastTemplateType]::ToastText02
    $toastXml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent($template)
    $textNodes = $toastXml.GetElementsByTagName('text')
    $null = $textNodes.Item(0).AppendChild($toastXml.CreateTextNode('Codex CLI'))
    $null = $textNodes.Item(1).AppendChild($toastXml.CreateTextNode($toastBody))

    $toast = [Windows.UI.Notifications.ToastNotification]::new($toastXml)
    if ($turnId) {
        $toast.Tag = $turnId
        $toast.Group = 'codex-cli-turns'
    }

    $notifier = [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Codex.CLI')
    $notifier.Show($toast)
} catch {
    Write-Verbose "codex-cli-notify: toast failed: $_"
}

