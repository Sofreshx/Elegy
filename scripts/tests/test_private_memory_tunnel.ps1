#requires -Version 7.2
<#
Runs only fictional local processes and files. No client, tunnel, key prompt or
network is used. Invoke with pwsh -NoProfile -File scripts/tests/test_private_memory_tunnel.ps1.
#>
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'These process supervision fixtures require Windows.' }
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$fixtureRoot = Join-Path $root ('target/qualification/tunnel-' + [Guid]::NewGuid().ToString('N'))
[void](New-Item -ItemType Directory -Path $fixtureRoot -Force)
$scriptPath = Join-Path $root 'scripts/start-private-memory-tunnel.ps1'
$tokens = $null; $parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile($scriptPath, [ref]$tokens, [ref]$parseErrors)
if ($parseErrors.Count -ne 0) { throw 'Tunnel helper does not parse.' }
foreach ($function in $ast.FindAll({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] }, $false)) {
    . ([scriptblock]::Create($function.Extent.Text))
}
function Assert-Fixture([bool]$Condition, [string]$Message) { if (-not $Condition) { throw $Message } }
$passed = 0
$secure = ConvertTo-SecureString 'FICTIONAL-CREDENTIAL-FIXTURE-ONLY' -AsPlainText -Force
$child = $null; $grandchild = $null
try {
    # Run a copy so a regression cannot put status into the real source tree.
    # Both early failure and default SelfTest must leave the helper's folder
    # unchanged; SelfTest gets a fictional OS temp root via this process env.
    $sourceFixture = Join-Path $fixtureRoot 'source-copy'
    $tempFixture = Join-Path $fixtureRoot 'os-temp'
    [void](New-Item -ItemType Directory -Path $sourceFixture, $tempFixture)
    $copiedScript = Join-Path $sourceFixture 'start-private-memory-tunnel.ps1'
    Copy-Item -LiteralPath $scriptPath -Destination $copiedScript
    $unexpectedStatus = Join-Path $sourceFixture 'private-memory-launch-status.json'
    $boundaryFailures = @()
    $output = & $copiedScript -ClientSha256 'fictional-invalid-hash' 3>$null
    if (Test-Path -LiteralPath $unexpectedStatus) { $boundaryFailures += 'Early failure wrote into the source folder.' }
    # Remove only this known fictional status to test SelfTest independently.
    if (Test-Path -LiteralPath $unexpectedStatus) { Remove-Item -LiteralPath $unexpectedStatus }
    $oldTmp = $env:TMP; $oldTemp = $env:TEMP
    try {
        $env:TMP = $tempFixture; $env:TEMP = $tempFixture
        Assert-Fixture ([IO.Path]::GetTempPath().TrimEnd([IO.Path]::DirectorySeparatorChar) -eq $tempFixture) 'Default self-test temp root is not isolated.'
        $output = & $copiedScript -SelfTest
    } finally { $env:TMP = $oldTmp; $env:TEMP = $oldTemp }
    if (Test-Path -LiteralPath $unexpectedStatus) { $boundaryFailures += 'SelfTest wrote into the source folder.' }
    $selfTestStates = @(Get-ChildItem -LiteralPath $tempFixture -Filter 'private-memory-launch-status.json' -Recurse)
    if ($selfTestStates.Count -ne 1) { $boundaryFailures += 'SelfTest did not create exactly one isolated status receipt.' }
    if ($selfTestStates.Count -eq 1) {
        $selfTestState = Get-Content -LiteralPath $selfTestStates[0].FullName -Raw | ConvertFrom-Json
        if ($selfTestState.stage -ne 'SELFTEST_PASS' -or $selfTestState.realCredentialUsed -or $selfTestState.networkStarted) { $boundaryFailures += 'Default SelfTest receipt is invalid.' }
    }
    Assert-Fixture ($boundaryFailures.Count -eq 0) ($boundaryFailures -join ' ')
    $passed += 2

    $explicitScratch = Join-Path $fixtureRoot 'explicit-selftest'
    [void](New-Item -ItemType Directory -Path $explicitScratch)
    $output = & $copiedScript -SelfTest -SelfTestDirectory $explicitScratch
    $explicitState = Get-Content -LiteralPath (Join-Path $explicitScratch 'private-memory-launch-status.json') -Raw | ConvertFrom-Json
    Assert-Fixture ($explicitState.stage -eq 'SELFTEST_PASS') 'Explicit fictional scratch self-test failed.'
    $rejected = $false
    try { $output = & $copiedScript -SelfTest -SelfTestDirectory $sourceFixture } catch { $rejected = $true }
    Assert-Fixture ($rejected -and -not (Test-Path -LiteralPath $unexpectedStatus)) 'SelfTest accepted its source folder as scratch space.'
    $passed++

    # Validation must preserve the separator that makes a drive root absolute.
    $rootCheck = Join-Path $fixtureRoot 'fictional-root-check.ps1'
    $statusFunction = $ast.FindAll({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Assert-StatusDirectory' }, $false)[0].Extent.Text
    [IO.File]::WriteAllText($rootCheck, 'param([string]$RootPath)' + [Environment]::NewLine + $statusFunction + [Environment]::NewLine + 'Assert-StatusDirectory $RootPath')
    $validatedRoot = & $rootCheck ([IO.Path]::GetPathRoot($fixtureRoot))
    Assert-Fixture ([IO.Path]::IsPathFullyQualified($validatedRoot)) 'Status validation converted an absolute drive root into a drive-relative path.'

    # A status I/O failure is reported generically and cannot replace cleanup.
    $statusPath = Join-Path $fixtureRoot 'missing/status.json'
    $child = New-OwnedProcess (Join-Path $PSHOME 'pwsh.exe') @('-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 60') $secure @{} $fixtureRoot
    $warning = @()
    try { $warning = @(Write-State 'FICTIONAL_FAILURE' @{} -BestEffort 3>&1) }
    finally { Stop-Owned $child }
    Assert-Fixture $child.HasExited 'Status failure prevented owned-process termination.'
    Assert-Fixture ($warning.Count -eq 1 -and -not ($warning -join '').Contains($fixtureRoot)) 'Status failure warning is missing or exposes a raw path.'
    $child.Dispose(); $child = $null; $passed++

    # Credential is in the child environment only. Parent secrets/proxies are
    # removed by the explicit environment allowlist; output is drained.
    $previousSecret = [Environment]::GetEnvironmentVariable('OPENAI_API_KEY','Process')
    $previousProxy = [Environment]::GetEnvironmentVariable('HTTPS_PROXY','Process')
    try {
        $env:OPENAI_API_KEY = 'FICTIONAL-PARENT-SECRET'; $env:HTTPS_PROXY = 'http://fictional.invalid'
        $code = 'if ($env:CONTROL_PLANE_API_KEY -ne "FICTIONAL-CREDENTIAL-FIXTURE-ONLY" -or $env:OPENAI_API_KEY -or $env:HTTPS_PROXY) { exit 71 }; [Console]::Out.Write("x" * 1048576); [Console]::Error.Write("y" * 1048576); exit 0'
        # The expected digest keeps the fixture credential out of arguments.
        $digest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes('FICTIONAL-CREDENTIAL-FIXTURE-ONLY')))
        $code = $code.Replace('$env:CONTROL_PLANE_API_KEY -ne "FICTIONAL-CREDENTIAL-FIXTURE-ONLY"', '([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($env:CONTROL_PLANE_API_KEY)))) -ne "' + $digest + '"')
        $child = New-OwnedProcess (Join-Path $PSHOME 'pwsh.exe') @('-NoProfile','-NonInteractive','-Command',$code) $secure @{} $fixtureRoot
        Assert-Fixture (-not ($child.StartInfo.ArgumentList -join ' ').Contains('FICTIONAL-CREDENTIAL-FIXTURE-ONLY')) 'Credential appeared in arguments.'
        Assert-Fixture (-not $child.StartInfo.Environment.ContainsKey('CONTROL_PLANE_API_KEY')) 'Credential remained in ProcessStartInfo.'
        Assert-Fixture ($child.WaitForExit(10000) -and $child.ExitCode -eq 0) 'Environment isolation or output drain failed.'
        $child.Dispose(); $child = $null; $passed++
    } finally {
        [Environment]::SetEnvironmentVariable('OPENAI_API_KEY',$previousSecret,'Process')
        [Environment]::SetEnvironmentVariable('HTTPS_PROXY',$previousProxy,'Process')
    }

    # Terminate a fictional tree, including the child created by our own child.
    $pidPath = Join-Path $fixtureRoot 'fictional-grandchild.pid'
    $exeLiteral = (Join-Path $PSHOME 'pwsh.exe').Replace("'", "''")
    $pidLiteral = $pidPath.Replace("'", "''")
    $code = '$p = Start-Process -FilePath ''' + $exeLiteral + ''' -ArgumentList @(''-NoProfile'',''-NonInteractive'',''-Command'',''Start-Sleep -Seconds 60'') -WindowStyle Hidden -PassThru; [IO.File]::WriteAllText(''' + $pidLiteral + ''', [string]$p.Id); $p.WaitForExit()'
    $child = New-OwnedProcess (Join-Path $PSHOME 'pwsh.exe') @('-NoProfile','-NonInteractive','-Command',$code) $secure @{} $fixtureRoot
    $deadline = [DateTimeOffset]::UtcNow.AddSeconds(10)
    while (-not (Test-Path -LiteralPath $pidPath) -and [DateTimeOffset]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 25 }
    Assert-Fixture (Test-Path -LiteralPath $pidPath) 'Fictional process tree did not become ready.'
    $grandchild = [Diagnostics.Process]::GetProcessById([int][IO.File]::ReadAllText($pidPath))
    Stop-Owned $child
    Assert-Fixture ($grandchild.WaitForExit(10000)) 'Owned descendant remained alive.'
    $child.Dispose(); $child = $null; $grandchild.Dispose(); $grandchild = $null; $passed++

    # The complete default path validates fictional pinned files without
    # starting either executable or reading a runtime credential.
    $client = Join-Path $fixtureRoot 'fictional-client.exe'; [IO.File]::WriteAllText($client,'not an executable')
    $memory = Join-Path $fixtureRoot 'fictional-memory.exe'; [IO.File]::WriteAllText($memory,'not an executable')
    $profile = Join-Path $fixtureRoot 'fictional.yaml'; [IO.File]::WriteAllText($profile,'name: fictional')
    $parameters = @{ ClientPath=$client; ClientSha256=(Get-FileHash -LiteralPath $client).Hash; MemoryPath=$memory; MemorySha256=(Get-FileHash -LiteralPath $memory).Hash; ProfileDirectory=$fixtureRoot; ProfileName='fictional'; ProfileSha256=(Get-FileHash -LiteralPath $profile).Hash; Database=(Join-Path $fixtureRoot 'fictional-memory.db'); AgentId='fictional-actor'; SharedSpace='fictional-space'; SharedAgentsJson='["fictional-actor"]' }
    $output = & $scriptPath @parameters
    $state = Get-Content -LiteralPath (Join-Path $fixtureRoot 'private-memory-launch-status.json') -Raw | ConvertFrom-Json
    Assert-Fixture ($state.stage -eq 'PREFLIGHT_PASS_NO_KEY_NO_NETWORK' -and -not $state.processStarted -and -not $state.keyRequested) 'Preflight unexpectedly started a process or requested a key.'
    Assert-Fixture (-not (Test-Path -LiteralPath $parameters.Database)) 'Preflight created a database.'
    $passed++
    [ordered]@{ status='PASS'; fixturesPassed=$passed; realCredentialUsed=$false; networkStarted=$false } | ConvertTo-Json -Compress
} finally {
    try { Stop-Owned $child } finally {
        if ($null -ne $grandchild) { Stop-Owned $grandchild; $grandchild.Dispose() }
        $secure.Dispose()
    }
    # Preserve fixture/status evidence under target/qualification for inspection.
}
