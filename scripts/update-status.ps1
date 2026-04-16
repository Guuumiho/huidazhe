param(
  [switch]$Force
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
$statusPath = Join-Path $repoRoot 'docs\STATUS.md'

if (-not (Test-Path $statusPath)) {
  throw "STATUS file not found: $statusPath"
}

$stagedFiles = git -C $repoRoot diff --cached --name-only --diff-filter=ACMR
if (-not $stagedFiles) {
  exit 0
}

$normalizedFiles = @(
  $stagedFiles | Where-Object { $_ -and $_.Trim() -ne '' }
)

$codePatterns = @(
  'web/',
  'src-tauri/',
  'README.md',
  'docs/ARCHITECTURE.md',
  'start.cmd',
  'start.ps1'
)

$hasTrackedCodeChange = $false
foreach ($file in $normalizedFiles) {
  foreach ($pattern in $codePatterns) {
    if ($file -like "$pattern*" -or $file -eq $pattern) {
      $hasTrackedCodeChange = $true
      break
    }
  }
  if ($hasTrackedCodeChange) {
    break
  }
}

if (-not $hasTrackedCodeChange) {
  exit 0
}

$timestamp = Get-Date -Format 'yyyy-MM-dd HH:mm'
$bulletLines = @(
  $normalizedFiles | ForEach-Object { "- $_" }
)

$entryLines = @(
  ''
  "### $timestamp"
  ''
  '- Auto maintenance note added before commit.'
  '- Staged files:'
)
$entryLines += $bulletLines

$current = Get-Content $statusPath -Raw

if ($current -notmatch '## 自动维护记录') {
  $current = $current.TrimEnd() + [Environment]::NewLine + [Environment]::NewLine + '## 自动维护记录'
}

$updated =
  $current.TrimEnd() +
  [Environment]::NewLine +
  [Environment]::NewLine +
  ($entryLines -join [Environment]::NewLine) +
  [Environment]::NewLine

[System.IO.File]::WriteAllText($statusPath, $updated, [System.Text.UTF8Encoding]::new($false))
