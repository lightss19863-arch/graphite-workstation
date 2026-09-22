# Graphite Workstation

Local-first legal intelligence workstation for solo practitioners and small law firms. Built with **Tauri v2** (Rust backend), **React** (frontend), and **SQLCipher** (encrypted local storage).

This is the public release repository. Source code for specific subsystems is available as standalone crates — see below.

**Download:** Check the [Releases](https://github.com/lightss19863-arch/graphite-workstation/releases) page for the latest Windows installer.

---

## What it actually does

Lawyers deal with sensitive client data — case strategy, witness statements, billing records. Most legal software either stores everything in someone else's cloud or has "encryption" that amounts to a password on a ZIP file.

Graphite keeps everything local by default. The workspace database is encrypted with SQLCipher (AES-256), the encryption key lives in the OS credential store (never on disk), and the cloud runtime can propose document changes but physically cannot write to your canonical records.

### The trust model in ~30 seconds

1. You ask the cloud to generate or analyze a legal document
2. The cloud produces a **proposal** and signs it with Ed25519
3. Your desktop **verifies the signature** against compiled-in trust keys
4. If verification passes, you can review and promote the proposal into your workspace
5. If it fails, the proposal is rejected. No override, no "accept anyway"

The cloud has zero write authority to your local database. It can't even read it.

---

## Architecture overview

```
┌─────────────────────────────────────────────────────┐
│  Desktop (Rust / Tauri v2)                          │
│                                                     │
│  ┌───────────────┐    ┌──────────────────────────┐  │
│  │ SQLCipher DB   │    │ Native Trust Verifier    │  │
│  │ (AES-256)      │    │ (Ed25519 / P-256)        │  │
│  └───────┬───────┘    └────────────┬─────────────┘  │
│          │                         │                 │
│          ▼                         ▼                 │
│  ┌─────────────────────────────────────────────┐    │
│  │ Run Outbox (state machine + event chain)     │    │
│  └─────────────────────┬───────────────────────┘    │
└────────────────────────┼────────────────────────────┘
                         │ signed egress
                         ▼
┌─────────────────────────────────────────────────────┐
│  Cloud Runtime (proposal-only, no write authority)   │
│  generates → signs → returns proposal                │
└─────────────────────────────────────────────────────┘
```

---

## Some of the harder problems I had to solve

### Keeping encryption keys out of the filesystem

The SQLCipher encryption key can't just sit in a config file — that defeats the entire point. So it goes into Windows Credential Manager / macOS Keychain via the `keyring` crate. But I hit a fun bug early on where a Windows domain policy update caused Credential Manager to silently succeed on write but return garbage on read. Now every key store operation does a read-after-write verification.

The workspace identity system also needs to survive folder renames. Each workspace gets a UUID manifest, and the key is stored under that UUID rather than the folder path. Learned that lesson when a tester moved their workspace folder and got permanently locked out.

→ Extracted as [`os-keyvault`](https://github.com/lightss19863-arch/os-keyvault)

### Cross-runtime Unicode encoding

This one was genuinely exhausting. The Rust backend, Node.js cloud runtime, and browser DOM all handle string indices differently:
- Rust counts bytes (UTF-8)
- JavaScript counts UTF-16 code units
- The DOM sometimes counts... something else depending on the API

For document redlining and diff tracking, all three runtimes need to agree on the exact character positions. I ended up standardizing everything on Unicode **code points** and converting at the boundaries. It works, but debugging off-by-one errors across three languages and two encoding schemes was not my idea of a good time.

### Deterministic receipt signing

When the cloud signs a receipt, both sides need to serialize the JSON payload identically before hashing. RFC 8785 (JSON Canonicalization Scheme) handles this, but the spec requires sorting object keys by UTF-16 code unit order, not UTF-8 byte order. The difference only matters for characters above U+FFFF (emoji, musical symbols), but when it does matter, it fails silently — you just get a different hash and the signature doesn't verify and you spend an afternoon wondering why.

→ Extracted as [`jcs-canonical-json`](https://github.com/lightss19863-arch/jcs-canonical-json)

### The outbox state machine

Cloud operations go through a state machine with CAS (compare-and-swap) revision control:

```
local_created → admission_pending → admitted → staging →
submitted → cloud_running → result_ready → downloaded → acknowledged
```

Every state transition is appended to an event chain with SHA-256 causation links, so if someone tampers with an event record, the chain verification catches it. The state machine is also designed to be crash-safe — if the app dies mid-transition, it resumes from the last committed state on restart.

Here's what the transition validation looks like (simplified from the actual Rust):

```rust
pub fn can_transition(from: &str, to: &str) -> bool {
    // terminal states can't go anywhere
    if matches!(from, "acknowledged" | "failed" | "cancelled" | "expired") {
        return false;
    }
    matches!(
        (from, to),
        ("local_created", "admission_pending")
            | ("admission_pending", "admitted")
            | ("admitted", "staging")
            | ("staging", "submitted")
            | ("submitted", "cloud_running")
            | ("cloud_running", "result_ready")
            | ("result_ready", "downloaded")
            | ("downloaded", "acknowledged")
            // any non-terminal state can fail, cancel, or expire
            | (_, "failed" | "cancelled" | "expired")
    )
}
```

---

## Technical stack

| Layer | Tech |
|---|---|
| Desktop runtime | Rust, Tauri v2 |
| Frontend | React, Vite |
| Local database | SQLite + SQLCipher (AES-256-CBC, HMAC-SHA512) |
| Key storage | OS credential store (keyring-rs) |
| Signatures | Ed25519 (RFC 8032), P-256 ECDSA (FIPS 186-4) |
| Key derivation | Argon2id (RFC 9106) — for disaster recovery passphrases |
| Serialization | RFC 8785 JCS for canonical signing |
| Cloud comms | Signed egress with monotonic revision control |

---

## Status

- Core desktop app: shipping, in production pilot
- Encrypted storage: shipping
- Cloud drafting pipeline: shipping
- Currently piloting with advocates at the MP High Court (Indore Bench)

---

## Related repos

- [`jcs-canonical-json`](https://github.com/lightss19863-arch/jcs-canonical-json) — RFC 8785 canonical JSON for Rust
- [`os-keyvault`](https://github.com/lightss19863-arch/os-keyvault) — OS credential store wrapper
- [`native-trust-verify`](https://github.com/lightss19863-arch/native-trust-verify) — JWK signature verification

## Contact

**Keshav Nagar** — [keshav@nyaya.cloud](mailto:keshav@nyaya.cloud) · [LinkedIn](https://www.linkedin.com/in/keshav-nagar-1709372a9) · [nyaya.cloud](https://nyaya.cloud)
