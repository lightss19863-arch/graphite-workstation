# Nyaya (Graphite) — Architecture & Systems Specification

```
Language: Rust (Host Engine) / TypeScript (Runtime & Contracts)
Security Boundary: Local-First / Zero Cloud Canonical State
Storage Engine: SQLCipher (AES-256-CBC) / Monotonic Row-Revision CAS Outbox
Cryptographic Standards: RFC 8785 JCS · Ed25519 · P-256 ECDSA · Argon2id · AES-256-GCM
```

---

## 1. Architectural Thesis

Standard generative AI systems treat Large Language Models as stateful, autonomous editors with direct database mutation privileges. In high-liability legal, compliance, and institutional domains, this paradigm fails due to non-deterministic hallucinations, lack of verifiable provenance, and silent state corruption.

**Nyaya enforces an inverted architectural invariant:**
> *Generative models are untrusted, proposal-only compilers. They possess zero canonical mutation authority and must operate within content-addressed, fail-closed state machines bounded by deterministic assertion graphs and cryptographic receipt trees.*

---

## 2. System Topology

Nyaya decouples local canonical custody from cloud compute via cryptographic outbox ledgers:

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                        DESKTOP HOST BOUNDARY (TAURI / RUST)                     │
│                                                                                 │
│  ┌───────────────────────┐   ┌────────────────────────┐   ┌──────────────────┐  │
│  │   OS Credential Vault │   │ SQLCipher B-Tree / WAL │   │ Native Trust     │  │
│  │   (DPAPI / Keychain)  │   │ (AES-256 Page Storage) │   │ Bundle (Ed25519) │  │
│  └───────────┬───────────┘   └───────────┬────────────┘   └────────┬─────────┘  │
│              │                           │                         │            │
│              ▼                           ▼                         ▼            │
│  ┌───────────────────────────────────────────────────────────────────────────┐  │
│  │                     Run Outbox Service (Schema v16)                       │  │
│  │   * Monotonic State Machine: local_created ──► admission ──► acknowledged │  │
│  │   * Row-Revision Compare-And-Swap (CAS) Concurrency Control               │  │
│  │   * Immutable SHA-256 Previous/Event Hash Chaining                        │  │
│  └─────────────────────────────────────┬─────────────────────────────────────┘  │
└────────────────────────────────────────┼────────────────────────────────────────┘
                                         │ Egress Protocol (RFC 8785 JCS Envelope)
                                         ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                   CLOUD RUNTIME (STATELESS PROPOSAL COMPILER)                   │
│                                                                                 │
│   ┌─────────────────────────────────────────────────────────────────────────┐   │
│   │ 5-Stage Content-Addressed Execution Pipeline                            │   │
│   │                                                                         │   │
│   │ [validate_bindings]                                                     │   │
│   │      │ (Input & Receipt Digest Verification)                            │   │
│   │      ▼                                                                  │   │
│   │ [generate_proposal]                                                     │   │
│   │      │ (Data-Only Prompt · Expiring PostgreSQL Leases · Usage Capping)  │   │
│   │      ▼                                                                  │   │
│   │ [deterministic_safety]                                                  │   │
│   │      │ (Assertion Support Graph · Source Span Coverage · Placeholder)   │   │
│   │      ▼                                                                  │   │
│   │ [sign_receipts]                                                         │   │
│   │      │ (Component Receipts ──► Subject Digest ──► Terminal Receipt)     │   │
│   │      ▼                                                                  │   │
│   │ [final_validation] ──► Emits ControlledDraftingResult (proposalOnly: true)│
│   └─────────────────────────────────────────────────────────────────────────┘   │
│                                                                                 │
│   Storage: Ephemeral AES-256-GCM Staging · Signed TTL Sweepers                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Core Systems Subsystems

### 3.1. The 5-Stage Checkpoint State Machine
The core drafting runtime (`ControlledDraftingStepExecutor`) processes generation through five immutable checkpoints:

1. **`validate_bindings`**: Cryptographically resolves input document versions, template digests, extraction span hashes, and prior research receipts. Verifies principal entitlements and execution manifests. Fails closed if any receipt signature is missing, forged, or expired.
2. **`generate_proposal`**: Dispatches execution through a provider-neutral Model Gateway under an isolated data-only context. Concurrency is governed by expiring PostgreSQL distributed leases with CAS takeover and conservative token/micros usage settlement.
3. **`deterministic_safety`**: Evaluates the **Assertion Support Graph**. Every substantive clause in the generated draft must mathematically link to authenticated source text or verified legal citations. Unproven propositions trigger explicit `unresolved:<clauseId>` issues; the model cannot bypass this check.
4. **`sign_receipts`**: Assembles a hierarchical cryptographic receipt tree:
   * **Component Receipts**: Signed per individual section and clause.
   * **Subject Receipt**: Ordered digest closure over all component hashes.
   * **Terminal Receipt**: Terminal binding closing the run ID, task digest, tenant, matter reference, issuer capability, and signing algorithm.
5. **`final_validation`**: Re-verifies all hashes, closes RFC 8785 canonical JSON digests, and tags the artifact with `noCanonicalMutation: true`.

```typescript
// cloud-runtime/src/controlled-drafting/execution.ts
const STAGES = [
  "validate_bindings",
  "generate_proposal",
  "deterministic_safety",
  "sign_receipts",
  "final_validation"
] as const;
```

---

### 3.2. Native Trust & Separation of Duties (The Four-Eyes Principle)
* **Zero Cloud Mutation**: The cloud runtime is physically barred from editing canonical matter records or updating document versions.
* **Native Rust Verification**: When the desktop downloads a generated proposal, `native_trust::verify` in Rust parses the detached Ed25519/P-256 signatures against compiled-in native trust bundles.
* **Separation of Duties**: Promoting a proposal into a canonical matter version requires a `ControlledDraftApprovalReceipt`. The runtime enforces that the requesting principal and the approving principal cannot be identical (`requester != approver`).
* **Atomic SQLite Promotion**: Promotion occurs in a single local SQLite transaction, updating the current version pointer, recording the audit event chain, and releasing the candidate state.

---

### 3.3. Deterministic Citation Verification Engine
* Evaluates case law citations across **6 independent deterministic facets**:
  1. **Existence**: Confirms authority identity in canonical legal corpora.
  2. **Passage Extraction**: Validates subspan extraction coordinates against canonical text.
  3. **Proposition Support**: Confirms linkage between retrieved evidence and cited propositions.
  4. **Authority Binding**: Validates jurisdiction, court level, and canonical metadata.
  5. **Currentness**: Evaluates subsequent treatment (overruled, distinguished, affirmed).
  6. **Applicability**: Verifies jurisdiction and temporal boundaries against matter facts.
* **Deterministic Fail-Closed Policy**: Negative or missing findings cannot be overridden by model confidence or reasoning heuristics.

---

### 3.4. Local Storage & Disaster Recovery (SQLCipher + Argon2id)
* **Encryption at Rest**: Workspaces are persisted in SQLite encrypted via SQLCipher with 256-bit AES-CBC. Database encryption keys are held in the operating-system credential store (Windows Credential Manager / macOS Keychain) and never written to the workspace directory.
* **Disaster Recovery**: Backup archives are encrypted using AES-256-GCM with key derivation handled by **Argon2id** (memory cost: 64MB, iterations: 3, parallelism: 4).
* **Integrity Enforcement**: At boot, the host executes SQLite `quick_check`, foreign key validation, and schema migration convergence checks. If corruption or tampering is detected, the engine enters a fail-closed `RECOVERY_REQUIRED` freeze state.

---

### 3.5. Cross-Runtime Unicode Code-Point Offsets
A critical engineering challenge in AST manipulation and redlining across disparate runtimes (Rust host, Node.js worker, browser DOM) is encoding drift:
* Standard JavaScript string indices count UTF-16 code units, causing coordinate drift when handling surrogate pairs, astral Unicode symbols (e.g. legal section signs `§`, paragraph symbols `¶`, non-Latin scripts).
* Nyaya specifies and enforces that **all text slicing, redline diff ranges, and AST extraction spans are computed in Unicode code points**.
* The Rust backend, Node.js runtime, and DraftPad virtualized editor share identical code-point coordinate systems, guaranteeing exact character synchronization without offset corruption.

---

## 4. Technical Specifications & Standards

```
Canonical Serialization    RFC 8785 JSON Canonicalization Scheme (JCS)
Digest Algorithm           SHA-256 (NIST FIPS 180-4)
Digital Signatures         Ed25519 (RFC 8032) / P-256 ECDSA (FIPS 186-4)
Key Derivation             Argon2id (RFC 9106)
Database Encryption        SQLCipher (256-bit AES-CBC, HMAC-SHA512 per page)
Outbox Concurrency         Monotonic Row-Revision Compare-And-Swap (CAS)
Distributed Leases         PostgreSQL Advisory Leases with Deadline Heartbeats
```

---

## 5. Security & Verification Invariants

1. **Zero Data Ingestion for Training**: Cloud runtime integrations operate under contractual zero-retention and zero-training enterprise terms. Prompts are tagged `data_only` and quarantined from executable instructions.
2. **Crash-Safe Outbox Operations**: State machine transitions are journaled in an append-only `run_outbox_events` table with monotonic sequence IDs and cryptographic causation links. Interrupted operations resume deterministically via idempotent replay.
3. **No Unbounded Retries**: Model dispatches are bounded by strict attempt limits and execution deadlines; unrecoverable semantic validation failures terminate execution immediately to prevent runaway resource consumption.

---

## 6. Project Status

* **Core Desktop Host**: Operational (Tauri v2 + Rust)
* **Encrypted Storage Engine**: Operational (SQLCipher v4)
* **Controlled Drafting Pipeline**: Milestone 8 Implemented
* **Citation Verification Subsystem**: Milestone 6 Implemented
* **Production Pilot**: Controlled pilot running for advocates practicing before the Madhya Pradesh High Court (Indore Bench).

**Platform Portal:** [nyaya.cloud](https://nyaya.cloud)  
**Security Research:** CrocodileSecurity (`keshav@nyaya.cloud`)  
**Founder:** [Keshav Nagar on LinkedIn](https://www.linkedin.com/in/keshav-nagar-1709372a9)
