param(
    [string]$CheckerScript = (Join-Path $PSScriptRoot "check-repo-shape.ps1")
)

$ErrorActionPreference = "Stop"
$taskRoot = (Resolve-Path -LiteralPath (Split-Path -Parent $PSScriptRoot)).Path
$tempRootPath = Join-Path $taskRoot ".tmp"
New-Item -ItemType Directory -Path $tempRootPath -Force | Out-Null
$tempRoot = (Resolve-Path -LiteralPath $tempRootPath).Path

function Remove-Fixture {
    param([string]$Path)
    $resolved = [System.IO.Path]::GetFullPath($Path)
    $prefix = $tempRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
    if (-not $resolved.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "refusing cleanup outside task .tmp root: $resolved"
    }
    if (Test-Path -LiteralPath $resolved) {
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
}

function Invoke-GitChecked {
    param([string[]]$Arguments)
    & git @Arguments
    if ($LASTEXITCODE -ne 0) { throw "git failed (exit $LASTEXITCODE): $($Arguments -join ' ')" }
}

function Invoke-Checker {
    param(
        [string]$Script,
        [string]$Project,
        [string]$UnreadablePath
    )

    $wrapper = Join-Path $tempRoot ("repo-shape-wrapper-" + [guid]::NewGuid().ToString("N") + ".ps1")
    @"
param([string]`$Checker, [string]`$Project, [string]`$UnreadablePath)
function Get-Content {
    [CmdletBinding()]
    param([string]`$LiteralPath, [switch]`$Raw)
    if (`$LiteralPath -eq `$UnreadablePath) {
        throw "synthetic unreadable fixture"
    }
    Microsoft.PowerShell.Management\Get-Content -LiteralPath `$LiteralPath -Raw:`$Raw
}
& `$Checker -Project `$Project -Json -FailOnIssues
exit `$LASTEXITCODE
"@ | Set-Content -LiteralPath $wrapper -NoNewline
    try {
        $output = & pwsh -NoProfile -File $wrapper -Checker $Script -Project $Project -UnreadablePath $UnreadablePath
        [pscustomobject]@{ Output = $output; ExitCode = $LASTEXITCODE }
    }
    finally {
        Remove-Fixture $wrapper
    }
}

if (-not (Test-Path -LiteralPath $CheckerScript)) { throw "checker script not found: $CheckerScript" }
$fixture = Join-Path $tempRoot ("repo-shape-fixture-" + [guid]::NewGuid().ToString("N"))
$plainFixture = Join-Path $tempRoot ("repo-shape-plain-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $fixture | Out-Null
try {
    Set-Content -LiteralPath (Join-Path $fixture "empty.txt") -Value "" -NoNewline
    Set-Content -LiteralPath (Join-Path $fixture "plain.txt") -Value "ordinary fixture text" -NoNewline
    $fakeLocalPath = "C:" + "/" + "Users" + "/" + "fixture-user" + "/" + "AppData" + "/" + "Local"
    Set-Content -LiteralPath (Join-Path $fixture "bad.txt") -Value $fakeLocalPath -NoNewline
    Invoke-GitChecked @("-c", "core.hooksPath=NUL", "init", "--quiet", "--", $fixture)
    Invoke-GitChecked @("-c", "core.hooksPath=NUL", "-C", $fixture, "add", "empty.txt", "plain.txt", "bad.txt")

    $json = & pwsh -NoProfile -File $CheckerScript -Project $fixture -Json -FailOnIssues
    $checkerExit = $LASTEXITCODE
    if ($checkerExit -ne 1) { throw "expected blocking local-path issue exit 1, got $checkerExit" }
    $report = $json | ConvertFrom-Json
    $localIssue = @($report.issues | Where-Object code -eq "content.local_path")
    if ($localIssue.Count -ne 1 -or $localIssue[0].path -ne "bad.txt") { throw "real non-empty local-path detection failed" }
    if (@($report.issues | Where-Object path -eq "empty.txt").Count -ne 0) { throw "empty tracked file produced an issue" }

    $unreadablePath = Join-Path $fixture "unreadable.txt"
    Set-Content -LiteralPath $unreadablePath -Value "ordinary fixture text" -NoNewline
    Invoke-GitChecked @("-c", "core.hooksPath=NUL", "-C", $fixture, "add", "unreadable.txt")
    $unreadableResult = Invoke-Checker -Script $CheckerScript -Project $fixture -UnreadablePath $unreadablePath
    if ($unreadableResult.ExitCode -ne 1) { throw "synthetic unreadable fixture unexpectedly passed (exit $($unreadableResult.ExitCode))" }
    $unreadableReport = $unreadableResult.Output | ConvertFrom-Json
    $unreadableIssue = @($unreadableReport.issues | Where-Object code -eq "content.unreadable")
    if ($unreadableIssue.Count -ne 1 -or $unreadableIssue[0].path -ne "unreadable.txt") { throw "unreadable tracked file was not reported" }
    if ($unreadableIssue[0].message -match "fixture|Users|AppData|Local") { throw "unreadable issue exposed fixture details" }

    New-Item -ItemType Directory -Path $plainFixture | Out-Null
    Set-Content -LiteralPath (Join-Path $plainFixture "plain.txt") -Value "ordinary fixture text" -NoNewline
    Invoke-GitChecked @("-c", "core.hooksPath=NUL", "init", "--quiet", "--", $plainFixture)
    Invoke-GitChecked @("-c", "core.hooksPath=NUL", "-C", $plainFixture, "add", "plain.txt")
    $plainJson = & pwsh -NoProfile -File $CheckerScript -Project $plainFixture -Json -FailOnIssues
    if ($LASTEXITCODE -ne 0) { throw "ordinary text fixture unexpectedly failed (exit $LASTEXITCODE)" }
    if (($plainJson | ConvertFrom-Json).issueCount -ne 0) { throw "ordinary text fixture reported issues" }
    "check-repo-shape fixture tests passed"
}
finally {
    Remove-Fixture $fixture
    Remove-Fixture $plainFixture
}
