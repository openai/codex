$ErrorActionPreference = 'Stop'

$payload = $env:CODEX_POWERSHELL_PAYLOAD
if ([string]::IsNullOrEmpty($payload)) {
    Write-Output '{"status":"parse_failed"}'
    exit 0
}

try {
    $source =
        [System.Text.Encoding]::Unicode.GetString(
            [System.Convert]::FromBase64String($payload)
        )
} catch {
    Write-Output '{"status":"parse_failed"}'
    exit 0
}

$tokens = $null
$errors = $null

$ast = $null
try {
    $ast = [System.Management.Automation.Language.Parser]::ParseInput(
        $source,
        [ref]$tokens,
        [ref]$errors
    )
} catch {
    Write-Output '{"status":"parse_failed"}'
    exit 0
}

if ($errors.Count -gt 0) {
    Write-Output '{"status":"parse_errors"}'
    exit 0
}

function Convert-CommandElement {
    param($element)

    if ($element -is [System.Management.Automation.Language.StringConstantExpressionAst]) {
        return @($element.Value)
    }

    if ($element -is [System.Management.Automation.Language.ExpandableStringExpressionAst]) {
        if ($element.NestedExpressions.Count -gt 0) {
            return $null
        }
        return @($element.Value)
    }

    if ($element -is [System.Management.Automation.Language.ConstantExpressionAst]) {
        return @($element.Value.ToString())
    }

    if ($element -is [System.Management.Automation.Language.CommandParameterAst]) {
        if ($element.Argument -eq $null) {
            return @('-' + $element.ParameterName)
        }

        if ($element.Argument -is [System.Management.Automation.Language.StringConstantExpressionAst]) {
            return @('-' + $element.ParameterName, $element.Argument.Value)
        }

        if ($element.Argument -is [System.Management.Automation.Language.ConstantExpressionAst]) {
            return @('-' + $element.ParameterName, $element.Argument.Value.ToString())
        }

        return $null
    }

    return $null
}

function Convert-PipelineElement {
    param($element)

    if ($element -is [System.Management.Automation.Language.CommandAst]) {
        if ($element.Redirections.Count -gt 0) {
            return $null
        }

        if (
            $element.InvocationOperator -ne $null -and
            $element.InvocationOperator -ne [System.Management.Automation.Language.TokenKind]::Unknown
        ) {
            return $null
        }

        $parts = @()
        foreach ($commandElement in $element.CommandElements) {
            $converted = Convert-CommandElement $commandElement
            if ($converted -eq $null) {
                return $null
            }
            $parts += $converted
        }
        return $parts
    }

    if ($element -is [System.Management.Automation.Language.CommandExpressionAst]) {
        if ($element.Redirections.Count -gt 0) {
            return $null
        }

        if ($element.Expression -is [System.Management.Automation.Language.ParenExpressionAst]) {
            $innerPipeline = $element.Expression.Pipeline
            if ($innerPipeline -and $innerPipeline.PipelineElements.Count -eq 1) {
                return Convert-PipelineElement $innerPipeline.PipelineElements[0]
            }
        }

        return $null
    }

    return $null
}

function Add-CommandsFromPipelineAst {
    param($pipeline, $commands)

    if ($pipeline.PipelineElements.Count -eq 0) {
        return $false
    }

    foreach ($element in $pipeline.PipelineElements) {
        $words = Convert-PipelineElement $element
        if ($words -eq $null -or $words.Count -eq 0) {
            return $false
        }
        $null = $commands.Add($words)
    }

    return $true
}

function Add-CommandsFromPipelineChain {
    param($chain, $commands)

    if (-not (Add-CommandsFromPipelineBase $chain.LhsPipelineChain $commands)) {
        return $false
    }

    if (-not (Add-CommandsFromPipelineAst $chain.RhsPipeline $commands)) {
        return $false
    }

    return $true
}

function Add-CommandsFromPipelineBase {
    param($pipeline, $commands)

    if ($pipeline -is [System.Management.Automation.Language.PipelineAst]) {
        return Add-CommandsFromPipelineAst $pipeline $commands
    }

    if ($pipeline -is [System.Management.Automation.Language.PipelineChainAst]) {
        return Add-CommandsFromPipelineChain $pipeline $commands
    }

    return $false
}

$commands = [System.Collections.ArrayList]::new()

foreach ($statement in $ast.EndBlock.Statements) {
    if (-not (Add-CommandsFromPipelineBase $statement $commands)) {
        $commands = $null
        break
    }
}

if ($commands -ne $null) {
    $normalized = [System.Collections.ArrayList]::new()
    foreach ($cmd in $commands) {
        if ($cmd -is [string]) {
            $null = $normalized.Add(@($cmd))
            continue
        }

        if ($cmd -is [System.Array] -or $cmd -is [System.Collections.IEnumerable]) {
            $null = $normalized.Add(@($cmd))
            continue
        }

        $normalized = $null
        break
    }

    $commands = $normalized
}

$result = if ($commands -eq $null) {
    @{ status = 'unsupported' }
} else {
    @{ status = 'ok'; commands = $commands }
}

,$result | ConvertTo-Json -Depth 3
