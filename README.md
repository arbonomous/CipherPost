# CipherPost

**Zero-Trust, Local-First, Privacy-Hardened Desktop Email Client for macOS (Apple Silicon)**

CipherPost connects directly from your Mac to your email providers via native Rust IMAP IDLE and SMTP—never routing credentials, metadata, or mail bodies through third-party cloud middlemen.

---

## Core Architecture

- **Framework**: Tauri v2 (Rust backend + React 19, TypeScript, Tailwind CSS v4 frontend) configured for native `aarch64-apple-darwin` builds.
- **Encrypted Local Storage & Instant Search**:
  - SQLite via `rusqlite` with **SQLCipher AES-256** encryption at rest (`kdf_iter = 256000`).
  - **SQLite FTS5** external-content virtual table (`messages_fts`) synced automatically via `AFTER INSERT`, `AFTER UPDATE`, and `AFTER DELETE` triggers for sub-20ms local full-text search across subjects, senders, recipients, headers, and message bodies.
- **Zero-Trust Credential Isolation**:
  - All IMAP/SMTP passwords, iCloud App Passwords, Proton Bridge secrets, Gmail OAuth2 tokens, and the 256-bit SQLCipher master key are stored exclusively in the **macOS Keychain** (`Security.framework` via `keyring`) and wrapped in `Zeroizing<String>` in RAM.
- **Active Privacy & Tracker Shield**:
  - Streaming Rust HTML sanitizer (`lol_html`) strips `<script>`, `<iframe>`, `<form>`, inline `on*` handlers, `javascript:` URIs, 1x1/hidden tracking pixels, known spy-pixel domains, and remote CSS `url()`/`@import` beacons before HTML reaches the WebView.
  - External images are blocked by default with a per-message **Load Remote Assets** toggle (which still keeps all trackers stripped).
  - Email bodies render inside strictly sandboxed iframes (`sandbox="allow-same-origin"` with `script-src 'none'` CSP).
- **Mail Protocols & PGP**:
  - `async-imap` + `tokio-rustls` two-phase sync (envelope headers first for instant inbox rendering, async `BODY.PEEK[]` streaming second) + background **IMAP IDLE** push listener.
  - `lettre` direct async SMTP delivery (Gmail OAuth2 `XOAUTH2`, iCloud App Passwords, Proton Bridge loopback `127.0.0.1:1143/1025`, and Custom IMAP/SMTP).
  - `sequoia-openpgp` (constant-time OpenSSL backend) MIME/inline PGP detection, detached signature verification, and decryption hooks.
- **On-Device AI Triage (Ollama / MLX)**:
  - Strictly loopback-enforced (`localhost` / `127.0.0.1` / `[::1]` only—cloud URLs are rejected by the Rust backend) for 2-bullet TL;DR summaries and context-aware reply drafting.

---

## Keyboard-First Tactical Navigation

| Key | Action |
|---|---|
| `j` / `k` or `↓` / `↑` | Navigate threads |
| `e` | Archive thread (or selected batch) to Vault |
| `r` | Reply to active thread |
| `c` | Compose new message |
| `x` | Select / toggle multi-select |
| `u` | Toggle read / unread |
| `s` | Star / unstar |
| `h` | Snooze until tomorrow |
| `#` / `Backspace` | Move to Trash |
| `Cmd+K` | Command Palette + sub-20ms FTS5 search |
| `/` | Focus inline FTS5 search |
| `g` then `i` / `v` / `s` / `z` / `t` | Jump to Inbox / Vault / Sent / Snoozed / Trash |
| `?` | Keyboard shortcuts overlay |

---

## Development & Apple Silicon Build

```bash
# Install frontend dependencies
pnpm install

# Run Rust unit tests (HTML Sanitizer, PGP Verification, SQLCipher + FTS5, Loopback AI Guard)
cargo test --manifest-path src-tauri/Cargo.toml

# Launch Tauri v2 desktop app in dev mode
pnpm tauri dev

# Build signed/native Apple Silicon (.app / .dmg)
pnpm tauri:build:macos
```
