$ErrorActionPreference = "Stop"

if (-not $env:AGT_TEST_REAL_GIT) {
  throw "AGT_TEST_REAL_GIT is not set"
}

Write-Host "Diagnosing git.exe from runner PATH: $env:AGT_TEST_REAL_GIT"

& $env:AGT_TEST_REAL_GIT --version
if ($LASTEXITCODE -ne 0) {
  throw "git --version failed"
}

& $env:AGT_TEST_REAL_GIT --exec-path
if ($LASTEXITCODE -ne 0) {
  throw "git --exec-path failed"
}

cargo test -p agt --test windows_git_diag -- --nocapture
if ($LASTEXITCODE -ne 0) {
  throw "windows git diagnostic test failed"
}
