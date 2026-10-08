param(
    [int]$CorpusSize = 750,
    [int]$ConcurrentWriters = 20,
    [string]$Root = "C:\Projects\Yad-Stress"
)

$ErrorActionPreference = "Stop"
$Yad = (Resolve-Path (Join-Path $PSScriptRoot "..\target\release\yad.exe")).Path
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)
$StartedAt = Get-Date

function Invoke-Yad {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
    & $Yad @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "yad failed ($LASTEXITCODE): $($Arguments -join ' ')"
    }
}

function Invoke-YadJson {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Arguments)
    $raw = & $Yad --json @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "yad failed ($LASTEXITCODE): $($Arguments -join ' ')"
    }
    return (($raw -join [Environment]::NewLine) | ConvertFrom-Json)
}

function Write-Utf8 {
    param([string]$Path, [string]$Text)
    [IO.File]::WriteAllText($Path, $Text, $Utf8NoBom)
}

function Write-MemoryFixture {
    param(
        [string]$Id,
        [string]$Kind,
        [string]$Space,
        [string]$Subject,
        [string]$Text,
        [int]$Importance = 3,
        [string]$Status = "active"
    )

    $path = Join-Path $Root ".yad\memory\2026\10\$Id.md"
    New-Item -ItemType Directory -Path (Split-Path $path -Parent) -Force | Out-Null
    $subjectYaml = $Subject.Replace("'", "''")
    $textBody = $Text.Trim()
    $front = @"
---
yad: 1
schema: memory
schema_version: 1
id: $Id
kind: $Kind
space: $Space
subject: '$subjectYaml'
status: $Status
importance: $Importance
source: stress-fixture
created: 2026-10-08T00:00:00Z
updated: 2026-10-08T00:00:00Z
tags:
- stress
---

$textBody
"@
    Write-Utf8 -Path $path -Text $front
}

function Percentile {
    param([double[]]$Values, [double]$P)
    if ($Values.Count -eq 0) { return 0 }
    $sorted = @($Values | Sort-Object)
    $index = [Math]::Ceiling(($P / 100.0) * $sorted.Count) - 1
    $index = [Math]::Max(0, [Math]::Min($index, $sorted.Count - 1))
    return [Math]::Round([double]$sorted[$index], 2)
}

Remove-Item -LiteralPath $Root -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Path $Root -Force | Out-Null
Set-Location $Root
git init | Out-Null
git config user.name "Yad Stress"
git config user.email "yad-stress@example.local"

Invoke-Yad init --name "Yad Stress"
Invoke-Yad infra up | Out-Null

$spaces = @(
    "stress/facilities",
    "stress/aps",
    "stress/security",
    "stress/storage",
    "stress/payments",
    "stress/membership",
    "stress/notifications",
    "stress/concurrency",
    "stress/noise"
)
foreach ($space in $spaces) {
    Invoke-Yad space add $space | Out-Null
}

$gold = @(
    @{
        Id = "MEM-STRESS-FACILITY"
        Kind = "decision"
        Space = "stress/facilities"
        Subject = "Manual facility approval"
        Text = "After an operator manually approves a facility request, the automatic worker must never re-evaluate it. Only an explicit operator re-review action can reopen the request for evaluation."
    },
    @{
        Id = "MEM-STRESS-APS"
        Kind = "failure"
        Space = "stress/aps"
        Subject = "APS timeout root cause"
        Text = "Increasing the HTTP timeout did not solve the APS integration incident. The actual root cause was downstream database connection pool exhaustion."
    },
    @{
        Id = "MEM-STRESS-TOKEN"
        Kind = "fact"
        Space = "stress/security"
        Subject = "طول عمر توکن"
        Text = "توکن دسترسی پانزده دقیقه اعتبار دارد و refresh token هفت روز معتبر است. تغییر این مقادیر نیازمند بازبینی سیاست امنیتی است."
    },
    @{
        Id = "MEM-STRESS-QDRANT"
        Kind = "decision"
        Space = "stress/storage"
        Subject = "Vector index source"
        Text = "Qdrant is a disposable derived semantic index. The tracked Markdown files under .yad are the source of truth and can rebuild the vector collection after loss."
    },
    @{
        Id = "MEM-STRESS-PAYMENT"
        Kind = "lesson"
        Space = "stress/payments"
        Subject = "پرداخت تکراری"
        Text = "callback پرداخت باید بر اساس کد پیگیری idempotent باشد؛ رسیدن دوباره callback نباید تراکنش یا امتیاز را دوباره ثبت کند."
    },
    @{
        Id = "MEM-STRESS-MOBILE"
        Kind = "decision"
        Space = "stress/membership"
        Subject = "Mobile number propagation"
        Text = "When an administrator changes a member mobile number, the change must be propagated to Membership and Identity. The next IMS synchronization must not silently overwrite the approved administrative change."
    },
    @{
        Id = "MEM-STRESS-CAPACITY"
        Kind = "fact"
        Space = "stress/facilities"
        Subject = "ظرفیت تسهیلات"
        Text = "تعداد درخواست‌های Approved یک دوره تسهیلات نباید از ظرفیت دوره بیشتر شود و پس از پر شدن ظرفیت، درخواست جدید باید مسدود شود."
    },
    @{
        Id = "MEM-STRESS-REDIS"
        Kind = "fact"
        Space = "stress/storage"
        Subject = "Redis role"
        Text = "Redis is only a cache and ephemeral coordination layer. PostgreSQL remains the authoritative transactional data store and Redis loss must be recoverable."
    },
    @{
        Id = "MEM-STRESS-SMS"
        Kind = "fact"
        Space = "stress/notifications"
        Subject = "Loan rejection SMS"
        Text = "پیامک رد درخواست تسهیلات باید نام تسهیلات و دلیل دقیق رد شدن مانند بدهی عضویت یا مغایرت نمایندگی را به زبان قابل فهم برای عضو اعلام کند."
    },
    @{
        Id = "MEM-STRESS-CRLF"
        Kind = "lesson"
        Space = "stress/storage"
        Subject = "Schema hash CRLF"
        Text = "Schema lock hashing must normalize CRLF and LF line endings, otherwise a Windows Git checkout can falsely report schema drift even when the schema meaning is unchanged."
    }
)

foreach ($item in $gold) {
    Write-MemoryFixture -Id $item.Id -Kind $item.Kind -Space $item.Space -Subject $item.Subject -Text $item.Text -Importance 5
}

$modules = @("inventory", "reporting", "education", "images", "calendar", "billing", "profile", "export")
$verbs = @("checks", "loads", "refreshes", "validates", "prepares", "renders", "exports", "archives")
$objects = @("draft items", "temporary rows", "preview data", "cached pages", "daily summaries", "pending forms", "thumbnail metadata", "test fixtures")

for ($i = 1; $i -le $CorpusSize; $i++) {
    $module = $modules[$i % $modules.Count]
    $verb = $verbs[($i * 3) % $verbs.Count]
    $object = $objects[($i * 5) % $objects.Count]

    if (($i % 7) -eq 0) {
        $hard = "A background worker may re-evaluate draft approval candidates before any final operator approval. Network timeout settings for preview downloads are unrelated to APS incidents."
    } elseif (($i % 11) -eq 0) {
        $hard = "The cache can be rebuilt and temporary index rows may be deleted during test runs, but this fixture does not define the project's source-of-truth policy."
    } else {
        $hard = "This synthetic note is unrelated to final business decisions."
    }

    $id = "MEM-STRESS-NOISE-{0:D5}" -f $i
    $subject = "Synthetic $module fixture $i"
    $text = "Operational fixture ${i}: the $module component $verb $object during deterministic stress batch $i. $hard Correlation token NX-$i."
    Write-MemoryFixture -Id $id -Kind "note" -Space "stress/noise" -Subject $subject -Text $text -Importance 1
}

$adrArgs = @(
    "--json", "adr", "new", "Manual facility approval is final",
    "--space", "stress/facilities",
    "--context", "Manual facility approval is an explicit operator action with business significance.",
    "--decision", "Approved requests are excluded from automatic worker evaluation unless an operator explicitly starts re-review.",
    "--consequences", "Automation cannot silently reverse a manual approval."
)
$adrRaw = & $Yad @adrArgs
if ($LASTEXITCODE -ne 0) { throw "failed to create stress ADR" }
$adr = (($adrRaw -join [Environment]::NewLine) | ConvertFrom-Json)
Invoke-Yad adr accept $adr.id | Out-Null
$GoldAdrId = $adr.id

$syncWatch = [Diagnostics.Stopwatch]::StartNew()
Invoke-Yad sync | Out-Null
$syncWatch.Stop()

$queries = @(
    @{ Query = "چرا درخواست تایید دستی دوباره توسط worker بررسی نمی‌شود؟"; Expected = $GoldAdrId; Name = "fa-to-en-authoritative-adr" },
    @{ Query = "What was the real cause when increasing the APS timeout failed?"; Expected = "MEM-STRESS-APS"; Name = "en-failure" },
    @{ Query = "access token چند دقیقه اعتبار دارد و refresh token چند روز؟"; Expected = "MEM-STRESS-TOKEN"; Name = "mixed-security" },
    @{ Query = "Where can the semantic vector collection be rebuilt from after Qdrant is lost?"; Expected = "MEM-STRESS-QDRANT"; Name = "en-storage" },
    @{ Query = "اگر callback پرداخت دوبار برسد چطور جلوی ثبت دوباره را بگیریم؟"; Expected = "MEM-STRESS-PAYMENT"; Name = "fa-payment" },
    @{ Query = "What must happen after an administrator changes a member mobile number?"; Expected = "MEM-STRESS-MOBILE"; Name = "en-membership" },
    @{ Query = "بعد از پر شدن ظرفیت دوره تسهیلات چه اتفاقی برای درخواست جدید می‌افتد؟"; Expected = "MEM-STRESS-CAPACITY"; Name = "fa-capacity" },
    @{ Query = "Is Redis the authoritative transactional data store?"; Expected = "MEM-STRESS-REDIS"; Name = "en-cache" },
    @{ Query = "پیامک رد وام باید چه اطلاعاتی به عضو بگوید؟"; Expected = "MEM-STRESS-SMS"; Name = "fa-sms" },
    @{ Query = "Why must schema hashes treat CRLF and LF as equivalent?"; Expected = "MEM-STRESS-CRLF"; Name = "en-crlf" }
)

$queryResults = @()
$latencies = @()
foreach ($case in $queries) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $result = Invoke-YadJson search $case.Query --limit 5
    $watch.Stop()
    $latencies += $watch.Elapsed.TotalMilliseconds

    $ids = @($result.results | ForEach-Object { $_.id })
    $zeroRank = [Array]::IndexOf($ids, $case.Expected)
    $rank = if ($zeroRank -ge 0) { $zeroRank + 1 } else { -1 }

    $top = if ($result.results.Count -gt 0) { $result.results[0] } else { $null }
    $queryResults += [PSCustomObject]@{
        name = $case.Name
        expected = $case.Expected
        rank = $rank
        hit1 = ($rank -eq 1)
        hit3 = ($rank -ge 1 -and $rank -le 3)
        latency_ms = [Math]::Round($watch.Elapsed.TotalMilliseconds, 2)
        top_id = if ($top) { $top.id } else { $null }
        top_semantic_score = if ($top) { $top.semantic_score } else { $null }
        semantic_used = $result.semantic_used
    }
}

$negativeQueries = @(
    "How do I bake sourdough bread with rye flour?",
    "What is the weather forecast for Amsterdam tomorrow?",
    "Which guitar strings are best for jazz?",
    "طرز تهیه قورمه سبزی برای چهار نفر چیست؟",
    "بهترین تمرین برای شنا کردن پروانه چیست؟"
)

$negativeResults = @()
foreach ($query in $negativeQueries) {
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $result = Invoke-YadJson search $query --limit 3
    $watch.Stop()
    $latencies += $watch.Elapsed.TotalMilliseconds
    $top = if ($result.results.Count -gt 0) { $result.results[0] } else { $null }
    $negativeResults += [PSCustomObject]@{
        query = $query
        result_count = @($result.results).Count
        top_id = if ($top) { $top.id } else { $null }
        top_semantic_score = if ($top) { $top.semantic_score } else { $null }
        latency_ms = [Math]::Round($watch.Elapsed.TotalMilliseconds, 2)
    }
}

$old = Invoke-YadJson remember "The stress lifecycle value is OLD-VALUE-ALPHA." --kind state --space stress/storage --subject "Lifecycle probe" --importance 5 --source stress
$superRaw = & $Yad --json memory supersede $old.id "The stress lifecycle value is NEW-VALUE-OMEGA." --kind state --subject "Lifecycle probe" --source stress --importance 5
if ($LASTEXITCODE -ne 0) { throw "memory supersede failed" }
$super = (($superRaw -join [Environment]::NewLine) | ConvertFrom-Json)
$currentNew = Invoke-YadJson search "NEW-VALUE-OMEGA" --limit 20
$currentOld = Invoke-YadJson search "OLD-VALUE-ALPHA" --limit 20
$historyOld = Invoke-YadJson search "OLD-VALUE-ALPHA" --limit 50 --include-history
$currentNewIds = @($currentNew.results | ForEach-Object { $_.id })
$currentOldIds = @($currentOld.results | ForEach-Object { $_.id })
$historyOldIds = @($historyOld.results | ForEach-Object { $_.id })
$lifecycleOk = (($currentNewIds -contains $super.new_id) -and -not ($currentOldIds -contains $old.id) -and ($historyOldIds -contains $old.id))

$dupText = "Exact duplicate probe value DUPLICATE-778899 should exist only once as active memory."
$dup1 = Invoke-YadJson remember $dupText --kind fact --space stress/storage --subject "Duplicate probe" --importance 3 --source stress
$dup2 = Invoke-YadJson remember $dupText --kind fact --space stress/storage --subject "Duplicate probe" --importance 3 --source stress
$allMemory = @(Invoke-YadJson memory list)
$duplicateCount = @($allMemory | Where-Object { $_.subject -eq "Duplicate probe" -and $_.status -eq "active" }).Count

$processes = @()
for ($i = 1; $i -le $ConcurrentWriters; $i++) {
    $stdout = Join-Path $Root "writer-$i.out"
    $stderr = Join-Path $Root "writer-$i.err"
    $argString = 'remember "Concurrent writer ' + $i + ' persisted unique marker CW-' + $i + '." --kind note --space stress/concurrency --subject "Concurrent writer ' + $i + '" --importance 1 --source stress'
    $processes += Start-Process -FilePath $Yad -ArgumentList $argString -WorkingDirectory $Root -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
}
foreach ($process in $processes) {
    $process.WaitForExit()
    $process.Refresh()
}
$concurrentSuccess = 0
for ($i = 1; $i -le $ConcurrentWriters; $i++) {
    $outPath = Join-Path $Root "writer-$i.out"
    if ((Test-Path $outPath) -and ((Get-Content $outPath -Raw) -match "Remembered|Already remembered")) {
        $concurrentSuccess++
    }
}
$concurrentFailure = $ConcurrentWriters - $concurrentSuccess

Write-MemoryFixture -Id "MEM-STRESS-EXTERNAL-PULL" -Kind "fact" -Space "stress/storage" -Subject "External pull probe" -Text "This fixture simulates a tracked memory arriving from git pull before local index synchronization." -Importance 3
$staleBeforeRemember = (Invoke-YadJson index status).stale
$staleProbe = Invoke-YadJson remember "Local write after simulated pull must not mark an incomplete semantic index current." --kind warning --space stress/storage --subject "Stale index probe" --importance 4 --source stress
$staleAfterRemember = (Invoke-YadJson index status).stale
$staleGuardOk = ($staleBeforeRemember -and $staleAfterRemember)
Invoke-Yad sync | Out-Null

$status = Invoke-YadJson status
$collection = $status.qdrant.collection
Invoke-RestMethod -Method Delete -Uri "http://127.0.0.1:6333/collections/$collection" | Out-Null
$fallback = Invoke-YadJson search "APS timeout connection pool exhaustion" --limit 5
$fallbackOk = (-not $fallback.semantic_used) -and (@($fallback.results | ForEach-Object { $_.id }) -contains "MEM-STRESS-APS")

$rebuildWatch = [Diagnostics.Stopwatch]::StartNew()
Invoke-Yad sync | Out-Null
$rebuildWatch.Stop()
$afterRebuild = Invoke-YadJson search "چرا زیاد کردن timeout مشکل APS را حل نکرد؟" --limit 5
$rebuildOk = $afterRebuild.semantic_used -and (@($afterRebuild.results | Select-Object -First 3 | ForEach-Object { $_.id }) -contains "MEM-STRESS-APS")

$corruptPath = Join-Path $Root ".yad\memory\2026\10\MEM-STRESS-CORRUPT.md"
$corruptText = "---" + [Environment]::NewLine + "schema: memory" + [Environment]::NewLine + "id: [" + [Environment]::NewLine + "---" + [Environment]::NewLine + "corrupt"
Write-Utf8 -Path $corruptPath -Text $corruptText
$previousErrorAction = $ErrorActionPreference
$ErrorActionPreference = "Continue"
$validationOutput = & $Yad validate 2>&1
$validationExitCode = $LASTEXITCODE
$ErrorActionPreference = $previousErrorAction
$corruptionCaught = ($validationExitCode -ne 0)
Write-Utf8 -Path (Join-Path $Root "corruption-validate.out") -Text ($validationOutput -join [Environment]::NewLine)
Remove-Item -LiteralPath $corruptPath -Force
Invoke-Yad sync | Out-Null

git add .yad | Out-Null
git commit -m "Stress corpus" | Out-Null
$cloneRoot = "$Root-Clone"
Remove-Item -LiteralPath $cloneRoot -Recurse -Force -ErrorAction SilentlyContinue
Set-Location (Split-Path $Root -Parent)
git clone $Root $cloneRoot | Out-Null
Set-Location $cloneRoot
$cloneBefore = Invoke-YadJson status
Invoke-Yad sync | Out-Null
$cloneAfter = Invoke-YadJson search "callback پرداخت دوباره ثبت نشود" --limit 5
$cloneHit = @($cloneAfter.results | Select-Object -First 3 | ForEach-Object { $_.id }) -contains "MEM-STRESS-PAYMENT"
$workspaceIsolation = ($cloneBefore.qdrant.collection -ne $collection)

$hit1 = @($queryResults | Where-Object hit1).Count
$hit3 = @($queryResults | Where-Object hit3).Count
$positiveScores = @($queryResults | Where-Object { $_.top_semantic_score -ne $null } | ForEach-Object { [double]$_.top_semantic_score })
$negativeScores = @($negativeResults | Where-Object { $_.top_semantic_score -ne $null } | ForEach-Object { [double]$_.top_semantic_score })

$report = [ordered]@{
    started_at = $StartedAt.ToString("o")
    completed_at = (Get-Date).ToString("o")
    corpus_size = $CorpusSize + $gold.Count + 1
    concurrent_writers = $ConcurrentWriters
    retrieval = @{
        cases = $queryResults
        hit_at_1 = [Math]::Round($hit1 / [double]$queries.Count, 4)
        hit_at_3 = [Math]::Round($hit3 / [double]$queries.Count, 4)
        latency_ms_p50 = Percentile -Values $latencies -P 50
        latency_ms_p95 = Percentile -Values $latencies -P 95
        positive_top_semantic_min = if ($positiveScores.Count) { [Math]::Round(($positiveScores | Measure-Object -Minimum).Minimum, 6) } else { $null }
        positive_top_semantic_avg = if ($positiveScores.Count) { [Math]::Round(($positiveScores | Measure-Object -Average).Average, 6) } else { $null }
    }
    unrelated_queries = @{
        cases = $negativeResults
        max_top_semantic_score = if ($negativeScores.Count) { [Math]::Round(($negativeScores | Measure-Object -Maximum).Maximum, 6) } else { $null }
        avg_top_semantic_score = if ($negativeScores.Count) { [Math]::Round(($negativeScores | Measure-Object -Average).Average, 6) } else { $null }
    }
    sync = @{
        initial_ms = [Math]::Round($syncWatch.Elapsed.TotalMilliseconds, 2)
        rebuild_after_collection_loss_ms = [Math]::Round($rebuildWatch.Elapsed.TotalMilliseconds, 2)
    }
    lifecycle = @{
        ok = $lifecycleOk
        old_id = $old.id
        new_id = $super.new_id
    }
    duplicate_memory = @{
        first_id = $dup1.id
        second_id = $dup2.id
        active_count = $duplicateCount
        deduplicated = ($duplicateCount -eq 1)
    }
    concurrency = @{
        requested = $ConcurrentWriters
        succeeded = $concurrentSuccess
        failed = $concurrentFailure
    }
    incremental_staleness = @{
        stale_after_external_change = $staleBeforeRemember
        remains_stale_after_local_write = $staleAfterRemember
        protected = $staleGuardOk
    }
    recovery = @{
        lexical_fallback_after_collection_loss = $fallbackOk
        semantic_rebuild_ok = $rebuildOk
    }
    corruption = @{
        malformed_memory_detected_by_validate = $corruptionCaught
    }
    team_clone = @{
        separate_workspace_collection = $workspaceIsolation
        semantic_search_after_clone_sync = $cloneHit
    }
}

$reportPath = Join-Path $Root "stress-report.json"
Write-Utf8 -Path $reportPath -Text ($report | ConvertTo-Json -Depth 12)
$report | ConvertTo-Json -Depth 12