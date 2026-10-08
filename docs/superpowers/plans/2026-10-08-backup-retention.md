# Automatic backups: retention policy and encryption — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automatic snapshots become backups with an on/off switch, an interval, a GFS / days /
count retention policy and optional encryption, all on the Mac.

**Architecture:** Retention is a pure domain module. Encryption is a new adapter (streaming
XChaCha20-Poly1305) plus a Keychain key; the app layer orchestrates clone → pack → encrypt for
automatic backups and decrypt → unpack for restores. The dashboard gets a new Backups section.

**Tech Stack:** Rust (axum, serde, tokio), crate `chacha20poly1305 = { version = "0.10.1",
features = ["stream"] }`, vanilla JS dashboard.

**Spec:** `docs/superpowers/specs/2026-10-08-backup-retention-design.md`

## Global Constraints

- No mocks, stubs or fakes in tests: real files, real APFS clones, a temporary Keychain
  (`security create-keychain`, see `TempKeychain` in `server/tests/adapters.rs:296`).
- Layering (enforced by `server/tests/architecture.rs`): domain does no I/O; adapters never use
  each other (`adapters::<name>`); only `app` uses adapters.
- Snapshots taken by hand (`auto: false`) are never pruned, never encrypted.
- Settings validation errors are shown verbatim in the dashboard: write them as user-facing
  sentences, like the existing ones in `domain/settings.rs`.
- Interval values: `[1, 5, 10, 15, 30, 60, 120, 240, 1440]` for settings; the per-VM override
  also accepts `0` (off). The dashboard offers 5, 15, 30, 60, 120, 240, 1440.
- Defaults: `enabled: true, every_min: 30, before_close: true, encryption: None,
  retention: Count { count: 4 }`; GFS defaults when the user picks it: 24/7/4/12/1.
- Keychain service for the key: `agentvm-backup-key`; env override `AGENTVM_BACKUP_KEY`
  (64 hex characters). Encrypted file name: `snapshot.tar.zst.enc`; magic `AGVMENC1`.
- Run the suites with `scripts/test.sh` (fast) and `scripts/test.sh --ignored` (real VMs).
  `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` must stay clean.

## Review Focus

- **An old `settings.json`** (`{"every_min":30,"keep":4,"before_close":true}`, or
  `every_min: 0`) must load into the new shape: `SettingsService::load` silently falls back to
  *all* defaults when parsing fails, so a broken migration would wipe every setting. Test in
  Task 2 parses a whole old settings file.
- **Local time:** a GFS "day" is a local day. A snapshot at 23:30 UTC with a +02:00 offset belongs
  to the next day. Test in Task 1.
- **A failure while encrypting** (e.g. no key in the Keychain and no env) must leave no listed
  snapshot and no plain disk behind, and the error must surface. Test in Task 5.
- **Restoring on a Mac without the key** must fail the launch with "this backup was encrypted on
  another Mac (or its file is damaged)", not panic or boot an empty disk. Test in Task 5.
- **A closed VM under a "last N days" policy** keeps its newest backup forever. Test in Tasks 1
  and 3.

---

### Task 1: Retention policies (pure domain)

**Files:**
- Create: `server/src/domain/retention.rs`
- Modify: `server/src/domain/mod.rs` (add `pub mod retention;`)
- Test: `server/tests/domain.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(tag = "kind", rename_all = "lowercase")]
  pub enum Retention {
      Gfs { hourly: u32, daily: u32, weekly: u32, monthly: u32, yearly: u32 },
      Days { days: u32 },
      Count { count: u32 },
  }
  impl Retention {
      pub const GFS_DEFAULT: Retention; // Gfs { 24, 7, 4, 12, 1 }
      pub fn validate(&self) -> Result<(), RetentionError>;
      /// Indices into `times` (seconds since the epoch, any order) to keep.
      pub fn keep(&self, times: &[f64], now: f64, utc_offset_s: i64) -> std::collections::BTreeSet<usize>;
  }
  #[derive(Debug, thiserror::Error, PartialEq, Eq)]
  pub enum RetentionError {
      #[error("GFS: each count must be between 0 and 1000, and at least one above 0")] Gfs,
      #[error("keep backups for 1 to 3650 days")] Days,
      #[error("keep between 1 and 50 backups")] Count,
  }
  ```

- [ ] **Step 1: Write the failing tests** in `server/tests/domain.rs`:
  - `gfs_keeps_the_newest_backup_of_each_recent_period`: one snapshot every 15 min from
    `2023-10-08T12:00Z` to `now = 2026-10-08T12:00Z` (a Thursday; the last snapshot is at `now`),
    offset 0, `GFS_DEFAULT` → `kept.len() == 43`: 24 hourly (12:00 and the :45 of the 23
    previous hours), 5 more daily (Oct 2–6 at 23:45), 3 more weekly (Sundays Oct 4, Sep 27,
    Sep 20 at 23:45), 11 more monthly (last day of each month Nov 2025 – Sep 2026 at 23:45),
    yearly adds nothing (2026's newest is `now`). Assert those exact timestamps are kept.
  - `gfs_counts_only_periods_that_have_backups`: snapshots on 3 distinct days 10 days apart,
    `Gfs { hourly: 0, daily: 7, weekly: 0, monthly: 0, yearly: 0 }` → all 3 kept.
  - `gfs_days_follow_the_local_time_zone`: snapshots at `2026-10-08T21:30Z` and
    `2026-10-08T22:30Z`, `Gfs { daily: 1, others 0 }`, `now` = `2026-10-08T23:00Z`.
    With offset `0` → one kept (same day). With offset `+7200` → the 22:30 one falls on the 9th
    locally, the 21:30 one is 23:30 on the 8th: daily 1 keeps only the newest (index of 22:30);
    with `daily: 2` both are kept.
  - `gfs_weeks_start_on_monday_across_years`: `2026-12-31` (Thursday) and `2027-01-03`
    (Sunday) are the same ISO week → `weekly: 1` keeps only the newer; `2027-01-04` (Monday)
    starts a new week.
  - `days_keeps_recent_backups_and_always_the_newest`: `Days { days: 2 }`, snapshots 1, 3 and 5
    days old → only the 1-day-old one; all 3 older than 2 days → only the newest of them.
  - `count_keeps_the_newest_n`: `Count { count: 2 }` on 4 unsorted times → the 2 largest.
  - `retention_is_validated`: `Gfs` all zeros → `Err(Gfs)`, `Gfs { hourly: 1001, .. }` →
    `Err(Gfs)`, `Days { 0 }` and `Days { 3651 }` → `Err(Days)`, `Count { 0 }` and
    `Count { 51 }` → `Err(Count)`, defaults → `Ok`.
  - `retention_serializes_with_a_kind`: `serde_json::to_value(Retention::Count { count: 4 })`
    == `{"kind":"count","count":4}`; `{"kind":"gfs","hourly":24,...}` round-trips.

- [ ] **Step 2: Run** `cd server && cargo test --test domain retention gfs days count` — expected:
  compile error (`retention` module missing).

- [ ] **Step 3: Implement `server/src/domain/retention.rs`.** Period keys from
  `local = floor(t) as i64 + utc_offset_s`: hour `local.div_euclid(3600)`, day
  `local.div_euclid(86400)`, ISO week `(day + 3).div_euclid(7)` (1970-01-01 is a Thursday, so
  this makes weeks start on Monday), month `y * 12 + m` and year `y` from the day number with the
  civil-from-days algorithm:

  ```rust
  fn civil(day: i64) -> (i64, u32) { // (year, month 1..=12)
      let z = day + 719_468;
      let era = z.div_euclid(146_097);
      let doe = z - era * 146_097;
      let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
      let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
      let mp = (5 * doy + 2) / 153;
      let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
      (yoe + era * 400 + i64::from(m <= 2), m)
  }
  ```

  GFS: sort indices newest first; for each granularity with count N walk them, keep an index
  when its period key differs from the last kept key of that granularity, stop after N. Union.
  `Days`: keep `now - t < days * 86400`. `Count`: newest `count`. Every policy finally inserts
  the index of the newest time (when `times` is not empty).

- [ ] **Step 4: Run** the same command — expected: all PASS.

- [ ] **Step 5: Commit** `feat: GFS, time period and count retention policies`.

### Task 2: Backup settings (domain shape, migration, callers)

**Files:**
- Modify: `server/src/domain/snapshot.rs` (`AutoSnapshots`, `SnapshotMeta`), `server/src/app/session.rs:112`,
  `server/src/app/snapshots.rs` (`take_auto`, `run_schedule`, `set_interval`),
  `server/tests/domain.rs:486-520` (replace the three old `AutoSnapshots` tests),
  `server/tests/system.rs:779`, `server/tests/http.rs`
- Test: `server/tests/domain.rs`, `server/tests/http.rs`

**Interfaces:**
- Consumes: `Retention`, `RetentionError` (Task 1).
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
  #[serde(rename_all = "lowercase")]
  pub enum Encryption { #[default] None, Aes256 }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
  #[serde(from = "AutoSnapshotsFile")] // private struct with the old and new fields as Options
  pub struct AutoSnapshots {
      pub enabled: bool, pub every_min: u32, pub before_close: bool,
      pub encryption: Encryption, pub retention: Retention,
  }
  impl AutoSnapshots {
      pub const INTERVALS: [u32; 9] = [1, 5, 10, 15, 30, 60, 120, 240, 1440];
      pub fn validate(&self) -> Result<(), AutoSnapshotsError>;
      pub fn due(&self, last_auto: Option<f64>, started_at: f64, now: f64) -> bool; // false when !enabled or every_min == 0
      pub fn to_prune(&self, all: &[SnapshotMeta], task: &str, now: f64, utc_offset_s: i64) -> Vec<SnapshotId>;
  }
  // AutoSnapshotsError: Interval ("backups run every 1, 5, 10, 15, 30, 60, 120 or 240 minutes, or once a day"),
  // Retention(#[from] RetentionError) — #[error(transparent)].
  // SnapshotMeta gains `#[serde(default)] pub encrypted: bool`.
  ```
  `set_interval` keeps accepting `0` plus `INTERVALS`.

- [ ] **Step 1: Write the failing tests.**
  - domain `old_backup_settings_are_migrated`: `{"every_min":30,"keep":4,"before_close":true}` →
    `enabled: true, every_min: 30, retention: Count { count: 4 }, encryption: None`;
    `{"every_min":0,"keep":7,"before_close":false}` → `enabled: false, every_min: 30,
    Count { 7 }`; `{}` → `AutoSnapshots::default()`.
  - domain `an_old_settings_file_still_loads_whole`: deserialize a full `Settings` JSON as
    written before this change (copy the shape of `tests/domain.rs:282` with the old
    `auto_snapshots`) → `Ok`, and `max_vms` keeps its non-default value.
  - domain `backups_are_due_every_interval_unless_disabled`: the old `due` assertions with the new
    struct, plus `enabled: false` → never due.
  - domain `pruning_follows_the_retention_and_never_touches_manual_ones`: the old pruning test
    with `retention: Count { count: 2 }`, plus a `Days { days: 1 }` case where every automatic
    snapshot is older than a day → only the newest automatic one survives, the manual one is
    never in the result.
  - domain `backup_settings_are_validated`: interval 7 → `Err(Interval)`, 1440 → `Ok`,
    retention `Count { 0 }` → `Err(Retention(Count))`.
  - http `backup_settings_round_trip_with_each_retention`: `PUT /api/settings` with
    `auto_snapshots.retention` set to each of the three kinds (and `encryption: "aes256"`) →
    200 and `GET` returns the same; `{"kind":"days","days":0}` → 400 whose `error` contains
    `"1 to 3650 days"`.

- [ ] **Step 2: Run** `scripts/test.sh` — expected: compile errors in tests.

- [ ] **Step 3: Implement** the shape and migration in `domain/snapshot.rs` (`AutoSnapshotsFile`
  with `enabled: Option<bool>`, `every_min: Option<u32>`, `keep: Option<u32>`,
  `before_close: Option<bool>`, `encryption: Option<Encryption>`,
  `retention: Option<Retention>`; mapping as in the spec's Migration bullet). `to_prune`: take
  this task's automatic snapshots, call `retention.keep`, return the others' ids. Callers:
  `session.rs` takes the before-close backup only when `enabled && before_close`;
  `run_schedule` and `take_auto` pass `now` and `utc_offset_s()` — extract the `date +%z`
  parsing of `clock` in `app/snapshots.rs` into `fn utc_offset_s() -> i64` and use it in both.
  Update `tests/system.rs:779` to `{"enabled":true,"every_min":1,"before_close":true,
  "encryption":"none","retention":{"kind":"count","count":1}}`.

- [ ] **Step 4: Run** `scripts/test.sh` — expected: all PASS.

- [ ] **Step 5: Commit** `feat: backup settings with retention policy, migrated from keep N`.

### Task 3: Hourly pruning, closed VMs included

**Files:**
- Modify: `server/src/app/snapshots.rs`
- Test: `server/tests/app.rs`

**Interfaces:**
- Consumes: `AutoSnapshots::to_prune` (Task 2), `utc_offset_s()` (Task 2).
- Produces: `pub fn prune_all(ctx: &AppCtx, now: f64)` — applies the retention to every
  `source_task` that has automatic snapshots.

- [ ] **Step 1: Write the failing test** `app.rs::pruning_reaches_backups_of_closed_vms`: with
  `ctx(home)`, set settings `retention: Days { days: 1 }`; create in
  `SnapshotStore::new(home.join("snapshots"))` (real `create` from a small temp `disk.raw` and
  `efivars`) three automatic snapshots of task `"gone"` created 5, 4 and 3 days ago and one
  manual snapshot 6 days ago; `prune_all(&ctx, now)` → only the 3-days-old automatic and the
  manual one remain.

- [ ] **Step 2: Run** `cd server && cargo test --test app pruning_reaches` — expected: FAIL
  (`prune_all` missing).

- [ ] **Step 3: Implement** `prune_all`; call it from `run_schedule` when an hour has passed
  since the last call (keep an `Instant` local to the loop; first call one minute after start).

- [ ] **Step 4: Run** the test — expected: PASS.

- [ ] **Step 5: Commit** `feat: retention also prunes backups of closed VMs, hourly`.

### Task 4: Encryption adapter and backup key

**Files:**
- Create: `server/src/adapters/crypt.rs`
- Modify: `server/Cargo.toml`, `server/src/adapters/mod.rs`, `server/src/adapters/keychain.rs`,
  `server/tests/architecture.rs:45` (add `"crypt"` to the list)
- Test: `server/tests/adapters.rs`

**Interfaces:**
- Produces:
  ```rust
  // adapters/crypt.rs
  pub const MAGIC: &[u8; 8] = b"AGVMENC1";
  #[derive(Debug, thiserror::Error)]
  pub enum CryptError { #[error("{0}")] Io(#[from] std::io::Error),
                        #[error("not an agentvm encrypted file")] Format,
                        #[error("wrong key or damaged file")] Auth }
  pub fn encrypt_file(key: &[u8; 32], src: &Path, dst: &Path) -> Result<(), CryptError>;
  pub fn decrypt_file(key: &[u8; 32], src: &Path, dst: &Path) -> Result<(), CryptError>;
  pub fn parse_key(hex: &str) -> Option<[u8; 32]>;   // exactly 64 hex characters
  // adapters/keychain.rs
  pub fn backup_key(&self) -> Result<Secret, KeychainError>;        // env, then Keychain; Err(MissingBackupKey)
  pub fn ensure_backup_key(&self) -> Result<Secret, KeychainError>; // creates 32 random bytes (hex) if missing
  ```
  File layout: `MAGIC`, 19-byte random nonce (`/dev/urandom`), then chunks of 1 MiB plaintext
  (+16-byte tag each) with `stream::EncryptorBE32<XChaCha20Poly1305>`; the last chunk (possibly
  empty) goes through `encrypt_last`. Decryption reads one chunk ahead to know which is last;
  a missing last chunk or any tag failure → `Auth`. `decrypt_file` writes to `dst` only through a
  temporary sibling renamed at the end, so a failed decryption leaves no `dst`.

- [ ] **Step 1: Write the failing tests** in `adapters.rs`:
  - `encryption_round_trips_a_multi_chunk_file`: 2.5 MiB of varied bytes → encrypt → file starts
    with `AGVMENC1`, differs from the input → decrypt → identical bytes. Also an empty file.
  - `a_wrong_key_or_a_changed_byte_is_refused`: other key → `Err(Auth)`; flip one byte in the
    middle → `Err(Auth)`; truncate the last 10 bytes → `Err(Auth)`; a file without the magic →
    `Err(Format)`; in every case `dst` does not exist.
  - `the_backup_key_is_created_once_in_the_keychain`: temp keychain → `backup_key()` is
    `Err(MissingBackupKey)`; `ensure_backup_key()` returns 64 hex characters; a second call and
    `backup_key()` return the same value.

- [ ] **Step 2: Run** `cd server && cargo test --test adapters encryption wrong_key backup_key` —
  expected: compile error.

- [ ] **Step 3: Implement** `crypt.rs`, the dependency, the Keychain functions (service
  `agentvm-backup-key`, env `AGENTVM_BACKUP_KEY` validated with the same rule as `parse_key`;
  new `KeychainError::MissingBackupKey` with message "no backup encryption key in the Keychain")
  and the architecture list entry.

- [ ] **Step 4: Run** `scripts/test.sh` — expected: all PASS (architecture included).

- [ ] **Step 5: Commit** `feat: streaming authenticated encryption and a backup key in the Keychain`.

### Task 5: Encrypted backups and their restore

**Files:**
- Modify: `server/src/adapters/snapshots.rs`, `server/src/app/snapshots.rs`,
  `server/src/app/supervisor.rs:219-223`
- Test: `server/tests/app.rs`, `server/tests/system.rs`

**Interfaces:**
- Consumes: `crypt::{encrypt_file, decrypt_file, parse_key}`, `Keychain::{backup_key, ensure_backup_key}` (Task 4),
  `archive::{pack, unpack}`, `SnapshotStore::scratch`.
- Produces:
  ```rust
  // adapters/snapshots.rs
  pub const SEALED: &str = "snapshot.tar.zst.enc";
  pub fn stage(&self, disk: &Path, efivars: &Path, name: &str) -> io::Result<PathBuf>; // scratch folder with disk.raw + efivars (clone)
  pub fn sealed(&self, id: &SnapshotId) -> io::Result<PathBuf>;  // creates <id>/, returns <id>/snapshot.tar.zst.enc
  pub fn commit(&self, meta: &SnapshotMeta) -> io::Result<SnapshotMeta>; // writes meta.json last; size from the folder
  // `adopt` also accepts a folder holding SEALED + meta.json.
  // app/snapshots.rs
  pub struct RestoreSource { pub disk: PathBuf, pub efivars: PathBuf, scratch: Option<PathBuf> } // Drop removes scratch
  pub async fn restore_source(ctx: &AppCtx, snap: &SnapshotId) -> Result<RestoreSource, SnapshotError>;
  /// Plain clone or, with Aes256, stage → pack → encrypt → commit. Used by `take`.
  pub async fn store_backup(ctx: &AppCtx, meta: SnapshotMeta, disk: &Path, efivars: &Path, encryption: Encryption) -> Result<SnapshotMeta, SnapshotError>;
  // SnapshotError gains `Encrypted` = "this backup was encrypted on another Mac (or its file is damaged)"
  ```
  `take(...)` for an automatic backup with `Encryption::Aes256`: `ensure_backup_key` → `stage` →
  `archive::pack(stage, scratch/x.tar.zst)` → `encrypt_file(key, archive, sealed(id))` →
  `commit(meta { encrypted: true })`; on any error remove the `<id>` folder and the scratch, and
  return the error. All of it inside `spawn_blocking`.
  `supervisor.rs` restore branch: `push_boot("host: decrypting the backup…")` when
  `meta.encrypted`, then `restore_source`, clone from its paths, drop it.

- [ ] **Step 1: Write the failing tests.**
  - app `an_encrypted_backup_restores_the_same_disk`: `ctx` with a temp keychain
    (`Keychain::new(Some(temp))`, created like `TempKeychain`), a 3 MiB `disk.raw` and an
    `efivars` file in a temp folder; `store_backup(&ctx, meta, &disk, &efivars, Aes256)` → the
    snapshot folder holds only `snapshot.tar.zst.enc` and `meta.json` with `encrypted: true`;
    `restore_source` gives a `disk` byte-identical to the original; after dropping it the scratch
    folder is gone.
  - app `encryption_failure_leaves_nothing_behind`: a `ctx` whose keychain path cannot be
    created (`/nonexistent/k.keychain-db`) → `store_backup(.., Aes256)` is `Err`, `list()` is
    empty, `snapshots/` holds no folder (neither the snapshot nor a scratch one).
  - app `a_backup_from_another_mac_says_so`: seal with keychain A, then build a second `ctx` on
    the same home with an empty keychain B → `restore_source` is `Err(Encrypted)`.
  - system `an_encrypted_backup_restores_into_a_new_vm` (`#[ignore]`, real VM): server with env
    `AGENTVM_BACKUP_KEY=<64 hex>`, settings `every_min: 1, encryption: "aes256"`; start a
    terminal, write `/root/work/marker.txt` through the shell PTY (reuse the helper used by
    `a_snapshot_restores_files_into_a_new_vm`), wait for an automatic snapshot with
    `encrypted: true`; restore it; the new VM's shell `cat`s `marker.txt`.

- [ ] **Step 2: Run** `cd server && cargo test --test app encrypted encryption_failure another_mac`
  — expected: compile error.

- [ ] **Step 3: Implement** as in Interfaces. `restore_source` for a plain snapshot returns the
  store's own paths with no scratch; for an encrypted one: `backup_key` (missing → `Encrypted`),
  `decrypt_file` into scratch (`Auth`/`Format` → `Encrypted`), `archive::unpack` into a scratch
  folder.

- [ ] **Step 4: Run** `scripts/test.sh`, then
  `cd server && cargo test --test system -- --ignored --exact an_encrypted_backup_restores_into_a_new_vm`
  and `a_snapshot_restores_files_into_a_new_vm` — expected: all PASS.

- [ ] **Step 5: Commit** `feat: encrypted automatic backups and their restore`.

### Task 6: Dashboard Backups section and docs

**Files:**
- Modify: `server/src/http/web/app.js:1080-1102` (settings section), the snapshot list rendering
  in `app.js` (lock icon), `server/src/http/web/app.css` (only if a new class is needed),
  `README.md` (feature table row "Snapshots, manual and automatic"), `CHANGELOG.md`

**Interfaces:**
- Consumes: the `auto_snapshots` JSON of Task 2 and `encrypted` on snapshots.

- [ ] **Step 1: Replace "Automatic snapshots" with "Backups"** in Settings, in this order: switch
  *Backups enabled*; select *Backup interval* (5 min, 15 min, 30 min, Hourly, Every 2 hours,
  Every 4 hours, Daily); switch *Backup before closing*; select *Encryption* ("None", "AES-256")
  with the hint "Encrypted with a key kept in this Mac's Keychain. Without it these backups
  cannot be opened. Encrypted backups are full archives: bigger and a few seconds slower.";
  select *Retention policy* with exactly the three labels "GFS (keep last N hourly, daily,
  weekly, monthly and yearly backups)", "Time period (last N days)", "Count (N last backups)";
  below it the fields of the chosen kind ("Hourly backups" … "Yearly backups"; "Days"; "Backups
  per machine"), and for GFS the link "What is GFS (Grandfather-Father-Son)?" toggling a
  `<p class="hint">`: "Keeps the newest backup of each of the last N hours, days, weeks, months
  and years. Recent work is kept in detail, older work less and less, so a long history takes
  little space." Switching kind fills that kind's defaults (GFS 24/7/4/12/1, 30 days, 4).
  Fields disabled while backups are off. Reuse the existing `this.set(...)`, switch and `unit`
  patterns of that section.

- [ ] **Step 2: Lock icon** in the snapshot list for `encrypted` snapshots, with title
  "Encrypted backup".

- [ ] **Step 3: Verify in the browser** with the `run` skill: change each field, save, reload the
  page, values persist; an invalid value (days 0) shows the server's message; the per-VM
  selector still works.

- [ ] **Step 4: Docs.** README row → "**Backups, manual and automatic** | … automatic backups
  on a schedule with a GFS, time-period or count retention, optionally encrypted with a key in
  the Keychain, plus one just before closing." CHANGELOG under Added: "Backup retention policies
  (GFS, time period, count) and optional encryption of automatic backups."

- [ ] **Step 5: Run** `scripts/test.sh` and commit `feat: Backups settings with retention policy and encryption`.
