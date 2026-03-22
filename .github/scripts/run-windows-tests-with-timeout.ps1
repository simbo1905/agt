$ErrorActionPreference = "Stop"

$cargoStdoutLog = Join-Path $env:RUNNER_TEMP "cargo-test-stdout.log"
$cargoStderrLog = Join-Path $env:RUNNER_TEMP "cargo-test-stderr.log"
$agtLog = Join-Path $env:RUNNER_TEMP "agt-debug.log"

if (Test-Path $cargoStdoutLog) {
  Remove-Item $cargoStdoutLog -Force
}
if (Test-Path $cargoStderrLog) {
  Remove-Item $cargoStderrLog -Force
}
if (Test-Path $agtLog) {
  Remove-Item $agtLog -Force
}

$env:AGT_DEBUG = "1"
$env:AGT_DEBUG_LOG = $agtLog

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
  RedirectStandardOutput = $cargoStdoutLog
  RedirectStandardError = $cargoStderrLog
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
  taskkill /PID $process.Id /T /F | Out-Host
}
else {
  $process.WaitForExit()
}

if (Test-Path $cargoStdoutLog) {
  Write-Host "--- cargo test stdout ---"
  Get-Content $cargoStdoutLog
}
else {
  Write-Host "No cargo test stdout log found"
}

if (Test-Path $cargoStderrLog) {
  Write-Host "--- cargo test stderr ---"
  Get-Content $cargoStderrLog
}
else {
  Write-Host "No cargo test stderr log found"
}

if (Test-Path $agtLog) {
  Write-Host "--- AGT debug log ---"
  Get-Content $agtLog
}
else {
  Write-Host "No AGT debug log found"
}

if ($timedOut) {
  exit 124
}

exit $process.ExitCode
