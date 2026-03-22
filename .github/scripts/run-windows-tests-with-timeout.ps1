$ErrorActionPreference = "Stop"

$agtLog = if ($env:AGT_LOG_PATH) { $env:AGT_LOG_PATH } else { Join-Path $env:RUNNER_TEMP "agt.log" }

if (Test-Path $agtLog) {
  Remove-Item $agtLog -Force
}

if (-not $env:AGT_TEST_REAL_GIT) {
  throw "AGT_TEST_REAL_GIT is not set"
}

Write-Host "Using git.exe for tests: $env:AGT_TEST_REAL_GIT"
& $env:AGT_TEST_REAL_GIT --version
if ($LASTEXITCODE -ne 0) {
  throw "AGT_TEST_REAL_GIT failed --version"
}


$arguments = @(
  "test",
  "--workspace",
  "--all-targets",
  "--all-features",
  "--",
  "--nocapture",
  "--test-threads=1"
)

Write-Host "Starting Windows full test suite with a 120s timeout"

$startProcessArgs = @{
  FilePath = "cargo"
  ArgumentList = $arguments
  NoNewWindow = $true
  PassThru = $true
}

$process = Start-Process @startProcessArgs

$deadline = (Get-Date).AddSeconds(120)
$timedOut = $false

while (-not $process.HasExited) {
  Start-Sleep -Seconds 5
  if ((Get-Date) -ge $deadline) {
    $timedOut = $true
    break
  }
}

if ($timedOut) {
  Write-Host "Cargo test timed out; terminating process tree"
  try {
    taskkill /PID $process.Id /T /F 2>&1 | Out-Host
  }
  catch {
    Write-Host "taskkill raised: $_"
  }
  try {
    $process.WaitForExit()
  }
  catch {
    Write-Host "wait after taskkill raised: $_"
  }
}
else {
  $process.WaitForExit()
}

if ($timedOut) {
  exit 124
}

exit $process.ExitCode
