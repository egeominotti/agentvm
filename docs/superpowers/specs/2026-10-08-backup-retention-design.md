# agentvm — Automatic backups: retention policy and encryption

Date: 2026-10-08 · Status: approved in chat · Extends the automatic snapshots of
`2026-10-08-agentvm-terminals-design.md`

## Goal

The automatic snapshots of running terminals become **backups** configured like a backup tool:
enabled, interval, encryption, and a **retention policy** (GFS, time period or count). They stay
**on the Mac only**; S3 remains a manual, per-snapshot action. Snapshots taken by hand are never
touched by any of this.

## Settings

`Settings.auto_snapshots` (`domain/snapshot.rs`) becomes:

```rust
pub struct AutoSnapshots {
    pub enabled: bool,              // default true
    pub every_min: u32,             // 5, 15, 30, 60, 120, 240, 1440 (no 0: `enabled` turns it off)
    pub before_close: bool,         // default true
    pub encryption: Encryption,     // None (default) | Aes256
    pub retention: Retention,
}
pub enum Retention {                // serde: {"kind": "gfs", ...}
    Gfs { hourly: u32, daily: u32, weekly: u32, monthly: u32, yearly: u32 },  // 24/7/4/12/1
    Days { days: u32 },             // 1..=3650
    Count { count: u32 },           // 1..=50
}
```

- **Migration** (serde, on load): an old file with `every_min: 0` → `enabled: false,
  every_min: 30`; `keep: N` → `Retention::Count { count: N }`. With no file the default is
  `enabled`, 30 min, before close, no encryption, `Count { count: 4 }`, so existing installs behave
  as today until the user changes something.
- **Validation:** interval in the list above; GFS counts 0..=1000 each and at least one above 0;
  days 1..=3650; count 1..=50.
- **Per-VM override** (`PUT /api/tasks/{id}/auto-snapshots`, `{every_min}` or `null`) stays;
  `every_min: 0` there still turns this VM's backups off.

## Retention (pure, `domain/snapshot.rs`)

`Retention::keep(&self, times: &[f64], now: f64) -> HashSet<usize>` returns the indices to keep
among one VM's automatic snapshots (seconds since the epoch, local time zone passed in as an
offset like the existing `clock`).

- **GFS:** for each granularity (hour, day, ISO week starting Monday, month, year) walk the
  snapshots newest first and keep the newest snapshot of each distinct period, until N periods
  are kept for that granularity. The kept set is the union over the granularities. Periods
  without snapshots do not count (a VM that ran on 3 days keeps those 3 daily ones).
- **Days:** keep snapshots with `now - t < days * 86400`.
- **Count:** keep the newest `count`.
- **Always:** the newest automatic snapshot of every VM is kept, so a closed VM never loses its
  last state.

`to_prune(all, task, now)` groups the automatic snapshots of `task`, applies `keep` and returns
the others. Manual snapshots (`auto: false`) are never candidates.

**When it runs:** after each automatic snapshot (for that VM), and once an hour from
`run_schedule` over every `source_task` that has automatic snapshots, including VMs already
closed (time-based policies need it).

## Encryption

When `encryption` is `Aes256`, an automatic backup is taken as today (clone of the disk, instant)
and then, in the background of the same task:

1. `archive::pack` the snapshot folder (`disk.raw`, `efivars`) to a `.tar.zst` in the store's
   scratch folder.
2. Encrypt it into `<snapshot>/snapshot.tar.zst.enc` with **XChaCha20-Poly1305 in STREAM mode**
   (crate `chacha20poly1305`, `stream::EncryptorBE32`, 1 MiB chunks; header: magic
   `AGVMENC1`, 19-byte random nonce). Any tampering or a wrong key fails the whole file.
3. Delete the plain `disk.raw` and `efivars` and the scratch archive. `meta.json` stays readable
   (name, date, VM, size) with a new field `encrypted: true`.

The snapshot appears in the list only once `meta.json` is written (last step), so a half-made
encrypted backup is never listed; `remove_leftovers` at start-up already cleans scratch folders.

- **Key:** 32 random bytes generated the first time encryption is turned on, stored hex-encoded
  in the Keychain (service `agentvm-backup-key`, same mechanism as the S3 secret). Turning
  encryption off keeps the key so existing encrypted backups still open. The Settings section
  warns: without this Mac's Keychain the encrypted backups cannot be opened.
- **Applies to automatic backups only.** Manual snapshots stay instant clones.
- **Restore:** decrypt and unpack into a scratch folder, then start the VM from it as from any
  snapshot (the scratch copy is deleted once the VM has its own clone). The dashboard shows
  "Decrypting…" while it happens.
- **Export / S3 backup / import** of an encrypted snapshot carry the encrypted file as is; an
  import on another Mac lists it, and restore there fails with "this backup was encrypted on
  another Mac" unless the key is the same.

## API and dashboard

- `GET`/`PUT /api/settings`: the new `auto_snapshots` shape; errors from validation as today.
- Settings → **Backups** (replaces "Automatic snapshots"): toggle *Backups enabled*, *Backup
  interval* (select), *Before close* (toggle), *Encryption* (None / AES-256, with the Keychain
  warning), *Retention policy* (select: "GFS (keep last N hourly, daily, weekly, monthly and
  yearly backups)", "Time period (last N days)", "Count (N last backups)") and the fields of the
  chosen one, plus the link "What is GFS (Grandfather-Father-Son)?" opening a short in-page
  explanation.
- Snapshot list: a lock icon on encrypted backups.
- The per-VM selector in the toolbar is unchanged.

## Testing (no mocks, as everywhere)

- **domain:** GFS over synthetic series (every 15 min for 3 years, gaps, month and year
  boundaries, ISO weeks across years), Days, Count, "newest always kept", validation, migration
  of old settings JSON.
- **adapters:** encryption round-trip on a multi-chunk file, wrong key fails, a flipped byte
  fails, a truncated file fails; Keychain key in a temporary keychain.
- **http:** settings round-trip with each retention kind; invalid values rejected with a message.
- **app:** hourly pruning removes old automatic snapshots of a closed VM and keeps manual ones.
- **system (real VM):** encrypted automatic backup → plain disk gone → restore → the VM has the
  file written before the backup.

## Out of scope

S3 as a backup destination, a user-chosen password, encryption of manual snapshots.
