$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$utf8 = [System.Text.UTF8Encoding]::new($false)
$stdin = [System.IO.StreamReader]::new([Console]::OpenStandardInput(), $utf8, $false)
$stdout = [System.IO.StreamWriter]::new([Console]::OpenStandardOutput(), $utf8)
$stdout.AutoFlush = $true

function Invoke-ParseRequest {
    param($RequestId, $Source)

    $tokens = $null
    $errors = $null

    $ast = $null
    try {
        $ast = [System.Management.Automation.Language.Parser]::ParseInput(
            $Source,
            [ref]$tokens,
            [ref]$errors
        )
    } catch {
        return @{ id = $RequestId; status = 'parse_failed' }
    }

    if ($errors.Count -gt 0) {
        return @{ id = $RequestId; status = 'parse_errors' }
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

    if ($commands -eq $null) {
        return @{ id = $RequestId; status = 'unsupported' }
    }

    return @{ id = $RequestId; status = 'ok'; commands = $commands }
}

function Write-Response {
    param($Response)

    $stdout.WriteLine(($Response | ConvertTo-Json -Compress -Depth 3))
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

    if ($pipeline.GetType().FullName -eq 'System.Management.Automation.Language.PipelineChainAst') {
        return Add-CommandsFromPipelineChain $pipeline $commands
    }

    return $false
}

# This script stays alive so the Rust caller can amortize PowerShell startup across
# many parse requests. Each request and response is one compact JSON line.
while (($requestLine = $stdin.ReadLine()) -ne $null) {
    $request = $null
    try {
        $request = $requestLine | ConvertFrom-Json
    } catch {
        Write-Response @{ id = $null; status = 'parse_failed' }
        continue
    }

    # We process requests serially, but still echo the id back so the Rust side can
    # detect protocol desyncs instead of silently trusting mixed stdout.
    $requestId = $request.id
    $payload = $request.payload
    if ([string]::IsNullOrEmpty($payload)) {
        Write-Response @{ id = $requestId; status = 'parse_failed' }
        continue
    }

    try {
        $source =
            [System.Text.Encoding]::Unicode.GetString(
                [System.Convert]::FromBase64String($payload)
            )
    } catch {
        Write-Response @{ id = $requestId; status = 'parse_failed' }
        continue
    }

    Write-Response (Invoke-ParseRequest -RequestId $requestId -Source $source)
}
