#requires -Version 7.2
[CmdletBinding()]
param(
    [string]$ClientPath, [string]$ClientSha256, [string]$MemoryPath, [string]$MemorySha256,
    [string]$ProfileDirectory, [string]$ProfileName, [string]$ProfileSha256, [string]$Database,
    [string]$AgentId, [string]$SharedSpace, [string]$SharedAgentsJson,
    [string]$BackupPath, [string]$BackupSha256,
    [switch]$LaunchApproved, [switch]$RuntimePermissionsVerified,
    [switch]$NoAuthenticationConfirmed, [switch]$SelfTest, [string]$SelfTestDirectory
)
$ErrorActionPreference = 'Stop'
if (-not $IsWindows) { throw 'Private Memory process supervision supports Windows only.' }
$statusPath = $null

function Write-State([string]$Stage, [hashtable]$Extra = @{}, [switch]$BestEffort) {
    $state = @{ stage = $Stage; observedAt = [DateTimeOffset]::UtcNow.ToString('o') }
    foreach ($key in $Extra.Keys) { $state[$key] = $Extra[$key] }
    try {
        if ([string]::IsNullOrWhiteSpace($statusPath)) { throw 'No validated status destination is available.' }
        Assert-NoReparseParents $statusPath $true
        [IO.File]::WriteAllText($statusPath, ($state | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
    } catch {
        if (-not $BestEffort) { throw }
        Write-Warning 'Controlled status could not be saved. Process cleanup still runs; review and revoke the dedicated key.' -WarningAction Continue
    }
}
function Assert-Hash([string]$Hash, [string]$Name) { if ($Hash -notmatch '^[0-9a-fA-F]{64}$') { throw "$Name must be a SHA-256 hex digest" } }
function Assert-Absolute([string]$Path, [string]$Name) {
    if ([string]::IsNullOrWhiteSpace($Path) -or -not [IO.Path]::IsPathFullyQualified($Path)) { throw "$Name must be an absolute path" }
    return [IO.Path]::GetFullPath($Path)
}
function Assert-NoReparseParents([string]$Path, [bool]$AllowMissingLeaf) {
    $full = [IO.Path]::GetFullPath($Path); $current = if (Test-Path -LiteralPath $full) { $full } else { [IO.Directory]::GetParent($full).FullName }
    while ($null -ne $current) {
        if (Test-Path -LiteralPath $current) {
            $item = Get-Item -LiteralPath $current -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Reparse point rejected: $current" }
        } elseif (-not $AllowMissingLeaf) { throw "Path does not exist: $current" }
        $parent = [IO.Directory]::GetParent($current); $current = if ($null -eq $parent) { $null } else { $parent.FullName }
    }
}
function Assert-PinnedFile([string]$Path, [string]$Hash, [string]$Name) {
    $full = Assert-Absolute $Path $Name; Assert-Hash $Hash $Name
    if (-not (Test-Path -LiteralPath $full -PathType Leaf)) { throw "$Name does not exist" }
    Assert-NoReparseParents $full $false
    if ((Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash -ne $Hash.ToUpperInvariant()) { throw "$Name hash mismatch" }
    return $full
}
function Assert-StatusDirectory([string]$Path) {
    $full = Assert-Absolute $Path 'StatusDirectory'
    if (-not (Test-Path -LiteralPath $full -PathType Container)) { throw 'Status directory must already exist.' }
    Assert-NoReparseParents $full $false
    $directory = $full.TrimEnd([IO.Path]::DirectorySeparatorChar)
    $source = [IO.Path]::GetFullPath($PSScriptRoot).TrimEnd([IO.Path]::DirectorySeparatorChar)
    if ($directory.Equals($source,[StringComparison]::OrdinalIgnoreCase) -or $directory.StartsWith($source + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Status cannot be written into the helper source folder.' }
    return $full
}
function New-OwnedProcess([string]$File, [string[]]$Arguments, [Security.SecureString]$Credential, [hashtable]$Environment, [string]$RuntimeHome) {
    $info = [Diagnostics.ProcessStartInfo]::new(); $info.FileName = $File; $info.WorkingDirectory = $RuntimeHome; $info.UseShellExecute = $false; $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true; $info.RedirectStandardError = $true; $info.Environment.Clear()
    foreach ($name in @('SystemRoot','WINDIR','PATH','PATHEXT','COMSPEC','NUMBER_OF_PROCESSORS')) { $value = [Environment]::GetEnvironmentVariable($name,'Process'); if ($null -ne $value) { $info.Environment[$name] = $value } }
    foreach ($name in @('USERPROFILE','HOME','APPDATA','LOCALAPPDATA','TEMP','TMP')) { $info.Environment[$name] = $RuntimeHome }
    foreach ($name in $Environment.Keys) { $info.Environment[$name] = $Environment[$name] }
    foreach ($arg in $Arguments) { [void]$info.ArgumentList.Add($arg) }
    $ptr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($Credential)
    try { $info.Environment['CONTROL_PLANE_API_KEY'] = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($ptr); $process = [Diagnostics.Process]::new(); $process.StartInfo = $info; [void]$process.Start() }
    finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($ptr) }
    [void]$info.Environment.Remove('CONTROL_PLANE_API_KEY')
    $null = $process.StandardOutput.BaseStream.CopyToAsync([IO.Stream]::Null); $null = $process.StandardError.BaseStream.CopyToAsync([IO.Stream]::Null)
    return $process
}
function Stop-Owned([Diagnostics.Process]$Process) {
    if ($null -eq $Process) { return }
    try { if (-not $Process.HasExited) { $Process.Kill($true) } } catch [InvalidOperationException] { }
    if (-not $Process.WaitForExit(10000)) { throw 'Owned process did not stop within ten seconds' }
}

if ($SelfTest) {
    $fixture = 'FICTIONAL-CREDENTIAL-SELFTEST-ONLY'; $secure = ConvertTo-SecureString $fixture -AsPlainText -Force; $child = $null
    try {
        if ([string]::IsNullOrWhiteSpace($SelfTestDirectory)) {
            $scratchRoot = Assert-StatusDirectory ([IO.Path]::GetTempPath())
            $scratch = Join-Path $scratchRoot ('elegy-memory-selftest-' + [Guid]::NewGuid().ToString('N'))
            [void](New-Item -ItemType Directory -Path $scratch)
        } else {
            $scratch = Assert-StatusDirectory $SelfTestDirectory
        }
        $scratch = Assert-StatusDirectory $scratch
        $statusPath = Join-Path $scratch 'private-memory-launch-status.json'
        $digest = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($fixture)))
        $code = 'if ([Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($env:CONTROL_PLANE_API_KEY))) -eq "' + $digest + '") { exit 0 } else { exit 71 }'
        $child = New-OwnedProcess (Join-Path $PSHOME 'pwsh.exe') @('-NoProfile','-NonInteractive','-Command',$code) $secure @{} $scratch
        if (($child.StartInfo.ArgumentList -join ' ').Contains($fixture)) { throw 'Fixture appeared in arguments' }
        if (-not $child.WaitForExit(10000) -or $child.ExitCode -ne 0) { throw 'Self-test environment transfer failed' }
        Write-State 'SELFTEST_PASS' @{ realCredentialUsed = $false; networkStarted = $false; actualTestStarted = $false; literalKeyInArguments = $false }
    } finally { try { Stop-Owned $child } finally { $secure.Dispose() } }
    return
}

$child = $null; $credential = $null; $locks = @(); $windowCompleted = $false; $runtimeHome = $null
try {
    foreach ($pair in @(@('ClientSha256',$ClientSha256),@('MemorySha256',$MemorySha256),@('ProfileSha256',$ProfileSha256))) { Assert-Hash $pair[1] $pair[0] }
    $profileDir = Assert-Absolute $ProfileDirectory 'ProfileDirectory'; Assert-NoReparseParents $profileDir $false
    if ([string]::IsNullOrWhiteSpace($ProfileName) -or $ProfileName -notmatch '^[a-zA-Z0-9_-]+$') { throw 'ProfileName must be a plain profile filename' }
    $client = Assert-PinnedFile $ClientPath $ClientSha256 'ClientPath'; $memory = Assert-PinnedFile $MemoryPath $MemorySha256 'MemoryPath'; $profile = Assert-PinnedFile (Join-Path $profileDir ($ProfileName + '.yaml')) $ProfileSha256 'Profile'
    $statusPath = Join-Path (Assert-StatusDirectory $profileDir) 'private-memory-launch-status.json'
    $database = Assert-Absolute $Database 'Database'; Assert-NoReparseParents $database $true
    if ([string]::IsNullOrWhiteSpace($AgentId) -or [string]::IsNullOrWhiteSpace($SharedSpace)) { throw 'AgentId and SharedSpace must be nonblank' }
    if (-not $SharedAgentsJson.Trim().StartsWith('[')) { throw 'SharedAgentsJson must be a JSON array' }
    $agents = @(ConvertFrom-Json $SharedAgentsJson); $badAgents = @($agents | Where-Object { $_ -isnot [string] -or [string]::IsNullOrWhiteSpace($_) -or $_ -ne $_.Trim() }); if ($agents.Count -eq 0 -or $badAgents.Count -gt 0) { throw 'SharedAgentsJson must be nonblank strings' }
    if (@($agents | Sort-Object -Unique).Count -ne $agents.Count -or $agents -notcontains $AgentId) { throw 'SharedAgentsJson must be distinct and include AgentId' }
    Write-State 'PREFLIGHT_PASS_NO_KEY_NO_NETWORK' @{ processStarted = $false; keyRequested = $false; networkStarted = $false }
    if (-not ($LaunchApproved -and $RuntimePermissionsVerified -and $NoAuthenticationConfirmed)) { 'Preflight passed. No key requested and no process started.'; return }
    if ([string]::IsNullOrWhiteSpace($BackupPath) -or [string]::IsNullOrWhiteSpace($BackupSha256)) { throw 'Launch requires a pinned BackupPath and BackupSha256' }
    $backup = Assert-PinnedFile $BackupPath $BackupSha256 'BackupPath'
    if (-not (Test-Path -LiteralPath $database -PathType Leaf) -or $backup -eq $database) { throw 'Launch requires the existing database and a distinct backup' }
    $runtimeHome = Join-Path $profileDir ('.runtime-home-' + [Guid]::NewGuid().ToString('N'))
    Assert-NoReparseParents $runtimeHome $true
    [void](New-Item -ItemType Directory -Path $runtimeHome)
    $identityTool = Join-Path $env:SystemRoot 'System32/fsutil.exe'
    $dbIdentity = (& $identityTool file queryfileid $database 2>$null | Out-String)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot verify database file identity' }
    $backupIdentity = (& $identityTool file queryfileid $backup 2>$null | Out-String)
    if ($LASTEXITCODE -ne 0) { throw 'Cannot verify backup file identity' }
    $dbId = [regex]::Match($dbIdentity, '0x[0-9a-fA-F]+').Value
    $backupId = [regex]::Match($backupIdentity, '0x[0-9a-fA-F]+').Value
    if (-not $dbId -or -not $backupId -or $dbId -eq $backupId) { throw 'Backup must have a distinct verified file identity' }
    foreach ($pinnedPath in @($client,$memory,$profile,$backup)) {
        $locks += [IO.File]::Open($pinnedPath,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
    }
    $credential = Read-Host 'Dedicated restricted runtime key (hidden input)' -AsSecureString
    if ($credential.Length -eq 0) { throw 'No runtime credential supplied' }
    $memoryEnvironment = @{ ELEGY_DB_PATH=$database; ELEGY_MCP_AGENT_ID=$AgentId; ELEGY_MCP_READ_SCOPE='agent'; ELEGY_MCP_SHARED_SPACE=$SharedSpace; ELEGY_MCP_SHARED_AGENTS=(ConvertTo-Json -InputObject $agents -Compress); ELEGY_EMBEDDING_BOOT_POLICY='prefer'; ELEGY_EMBEDDING_MODEL='nomic-embed-text'; OLLAMA_URL='http://127.0.0.1:11434' }
    $blocked = @('HTTP_PROXY','HTTPS_PROXY','ALL_PROXY','OPENAI_API_KEY','OPENAI_BASE_URL','ELEGY_ADMIN_TOKEN'); foreach ($name in $blocked) { [void]$memoryEnvironment.Remove($name) }
    $started = [DateTimeOffset]::UtcNow; $deadline = $started.AddMinutes(15)
    Write-State 'DOCTOR_STARTING' @{ processStarted = $false; keyRequested = $true; deadline = $deadline.ToString('o') }
    [void](Assert-PinnedFile $client $ClientSha256 'ClientPath')
    [void](Assert-PinnedFile $memory $MemorySha256 'MemoryPath')
    [void](Assert-PinnedFile $profile $ProfileSha256 'Profile')
    Assert-NoReparseParents $database $false
    $child = New-OwnedProcess $client @('doctor','--profile',$ProfileName,'--profile-dir',$profileDir) $credential $memoryEnvironment $runtimeHome
    if (-not $child.WaitForExit([Math]::Min(60000,[Math]::Max(1,[int](($deadline - [DateTimeOffset]::UtcNow).TotalMilliseconds))))) { Stop-Owned $child; throw 'Doctor did not complete within sixty seconds' }
    if ($child.ExitCode -ne 0) { throw 'Doctor failed; no connection claim made' }
    Write-State 'CLIENT_RUNNING_UNVERIFIED' @{ processStarted = $true; deadline = $deadline.ToString('o'); cloudExposureVerified = $false; revocationRequiredAfterStop = $true }
    [void](Assert-PinnedFile $client $ClientSha256 'ClientPath')
    [void](Assert-PinnedFile $memory $MemorySha256 'MemoryPath')
    [void](Assert-PinnedFile $profile $ProfileSha256 'Profile')
    Assert-NoReparseParents $database $false
    $child = New-OwnedProcess $client @('run','--profile',$ProfileName,'--profile-dir',$profileDir,'--health.url-file',(Join-Path $profileDir 'health.url')) $credential $memoryEnvironment $runtimeHome
    $credential.Dispose(); $credential = $null
    Write-State 'CLIENT_RUNNING_UNVERIFIED' @{ processStarted = $true; pid = $child.Id; deadline = $deadline.ToString('o'); cloudExposureVerified = $false }
    while (-not $child.WaitForExit(1000)) { if ([DateTimeOffset]::UtcNow -ge $deadline) { $windowCompleted = $true; Stop-Owned $child; break } }
} catch {
    Write-State 'STOP_REQUESTED_REVIEW_REVOCATION' @{ stopVerified = $false; rawErrorWithheld = $true; errorType = $_.Exception.GetType().Name; revocationRequiredAfterStop = $true } -BestEffort
    'Stopped or stopping. No raw authentication output displayed.'
} finally {
    $stopVerified = $true
    try { Stop-Owned $child } catch { $stopVerified = $false }
    if ($null -ne $credential) { $credential.Dispose() }
    foreach ($lockedFile in $locks) { $lockedFile.Dispose() }
    $homeRemoved = $false
    if ($stopVerified -and $null -ne $runtimeHome) {
        try {
            $ownedHome = [IO.Path]::GetFullPath($runtimeHome)
            $ownedRoot = [IO.Path]::GetFullPath($profileDir).TrimEnd([IO.Path]::DirectorySeparatorChar) + [IO.Path]::DirectorySeparatorChar
            if (-not $ownedHome.StartsWith($ownedRoot,[StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($ownedHome) -notmatch '^\.runtime-home-[0-9a-f]{32}$') { throw 'Unexpected cleanup path' }
            Assert-NoReparseParents $ownedHome $false
            if (@(Get-ChildItem -LiteralPath $ownedHome -Recurse -Force | Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 }).Count -gt 0) { throw 'Runtime-home reparse entry requires manual cleanup' }
            Remove-Item -LiteralPath $ownedHome -Recurse -Force
            $homeRemoved = -not (Test-Path -LiteralPath $ownedHome)
        } catch { $homeRemoved = $false }
    }
    if ($null -ne $child) {
        $stage = if (-not $stopVerified) { 'STOP_FAILED_REVOKE_KEY_IMMEDIATELY' } elseif ($windowCompleted) { 'WINDOW_ENDED_REVOKE_KEY' } else { 'EARLY_STOP_REVIEW_AND_REVOKE_KEY' }
        $exitCode = if ($stopVerified) { $child.ExitCode } else { $null }
        Write-State $stage @{ childWasStarted = $true; processStillRunning = (-not $stopVerified); runtimeHomeRemoved = $homeRemoved; pid = $child.Id; windowCompleted = $windowCompleted; stopVerified = $stopVerified; exitCode = $exitCode; cloudExposureVerified = $false; revocationVerified = $false } -BestEffort
        'Review controlled runtime status. Revoke the dedicated key and disconnect the private plugin; revocation is not verified here.'
    }
}
