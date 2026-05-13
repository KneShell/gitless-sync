# gitless-sync

Read-only CLI that compares a local directory to a GitHub repo and reports drift as a 4-class JSON summary. Designed for environments where `git` itself is unusable (e.g. iCloud-synced directories) and for AI callers that need facts, not decisions.

The tool does not modify files or remotes. Writes are out of scope by design (ADR 0001). All GitHub API access goes through the `gh` CLI as a subprocess (ADR 0001 + ADR 0002), so there is no token plumbing in this binary.

## Prerequisites

### `gh` CLI

Minimum version: **`gh >= 2.40.0`** (multi-account auth stabilized — see `docs/specs/spec-error-contracts.md` § 현재 상태).

| OS | Install |
|----|---------|
| Windows | `winget install --id GitHub.cli` |
| macOS | `brew install gh` |
| Linux (Debian/Ubuntu) | `sudo apt install gh` (or follow https://cli.github.com/ for the official apt repo) |
| Linux (Fedora/RHEL) | `sudo dnf install gh` |

Other distributions: see https://cli.github.com/.

### Authenticate once

```sh
gh auth login
```

Pick GitHub.com → HTTPS → "Login with a web browser" (or paste an existing PAT). gitless-sync inherits this auth — there is no `--token` flag and no `GITHUB_TOKEN` lookup.

### Rust toolchain (only to build from source)

`rust-toolchain.toml` pins MSRV `1.95.0`. `rustup` will fetch it on first build.

## Install (prebuilt binary)

Tag-pushed releases publish portable archives for three targets. Pick the asset that matches your platform, verify it, and put the binary on your `PATH`. Replace `v0.7.0` with the latest tag from the Releases page and `<owner>` with the repo owner in the URLs below.

| Platform | Archive |
|----------|---------|
| Windows (x86_64) | `gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip` |
| Linux (x86_64, musl static) | `gitless-sync-v0.7.0-x86_64-unknown-linux-musl.tar.gz` |
| macOS (Apple Silicon) | `gitless-sync-v0.7.0-aarch64-apple-darwin.tar.gz` |

Each archive ships the `gitless-sync` binary (`.exe` on Windows) plus `README.md`, `LICENSE-MIT`, `LICENSE-APACHE`, and `CHANGELOG.md`. Per-asset SHA-256 lives in `<archive>.sha256`; an aggregate `sha256sums.txt` covers every archive in the release.

### Download

```powershell
# Windows PowerShell
Invoke-WebRequest -Uri "https://github.com/<owner>/gitless-sync/releases/download/v0.7.0/gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip" -OutFile gitless-sync.zip
Expand-Archive gitless-sync.zip -DestinationPath gitless-sync
```

```sh
# Linux / macOS
curl -L -o gitless-sync.tar.gz "https://github.com/<owner>/gitless-sync/releases/download/v0.7.0/gitless-sync-v0.7.0-x86_64-unknown-linux-musl.tar.gz"
tar -xzf gitless-sync.tar.gz
```

### Verify SHA-256

```powershell
# Windows: hash the archive and compare against the matching .sha256 file shipped in the release.
Get-FileHash -Algorithm SHA256 gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip
```

```sh
# Linux / macOS: download the aggregate manifest and verify in one step.
curl -L -O "https://github.com/<owner>/gitless-sync/releases/download/v0.7.0/sha256sums.txt"
sha256sum -c sha256sums.txt --ignore-missing
```

### Verify attestation (SLSA build provenance)

Every archive carries a GitHub-signed SLSA attestation that proves it was built by this repo's release workflow. Requires `gh >= 2.40.0` (already a prerequisite above).

```sh
gh attestation verify gitless-sync-v0.7.0-x86_64-pc-windows-msvc.zip --repo <owner>/gitless-sync
```

A `Verified` line means the archive came out of this repo's `Release` workflow on a tagged build.

## Build from source

```sh
cargo build --release
```

Output: `target/release/gitless-sync` (or `gitless-sync.exe` on Windows).

## Quick Start

```sh
# Generate config file once per directory:
gitless-sync init --repo owner/name --branch main > gitless-sync.toml

# Then scan repeatedly without flags:
gitless-sync scan
```

`init` writes nothing on its own — it prints TOML to stdout and you redirect (ADR 0004). `scan` then reads `gitless-sync.toml` from the working directory, so subsequent runs need no flags.

## Usage

### `scan` — full directory comparison

```sh
gitless-sync scan --repo owner/name --local ./path/to/dir
```

Branch defaults to `main`. Override with `--branch <name>`.

Common flags:

```sh
# summary counts only; if failed > 0, also emits minimal {path, presence, failed_reason} entries — failed_reason always present incl. hash_io (LLM context budget)
gitless-sync scan --repo owner/name --local . --summary-only

# only files that drifted
gitless-sync scan --repo owner/name --local . --status drift

# multiple status filters, comma-separated
gitless-sync scan --repo owner/name --local . --status drift,local_only_changed

# extra ignore patterns on top of .gitignore (gitignore syntax, repeatable)
gitless-sync scan --repo owner/name --local . --ignore "*.tmp" --ignore "build/"

# pretty-printed output
gitless-sync scan --repo owner/name --local . --pretty

# stderr verbosity: -v info, -vv debug
gitless-sync scan --repo owner/name --local . -v
```

Output schema and the four status values (`identical`, `local_only_changed`, `remote_only_changed`, `drift`, plus `failed` for partial failures) are defined in `docs/specs/spec-output-schema.md`.

### `diff` — single-file diff

```sh
gitless-sync diff <relative/path> --repo owner/name --local .
```

Both sides are LF-normalized and BOM-stripped before diffing (use `--keep-bom` to preserve UTF-8 BOM).

#### Default output (unified text)

The shape of the output depends on which sides exist and whether they normalize to the same content:

| Case | stdout | stderr | exit |
|------|--------|--------|------|
| Both sides exist, normalize-equal | empty | empty | 0 |
| Both sides exist, normalize-diff | unified diff text (`--- a/...\n+++ b/...\n@@ ...`) | empty | 0 |
| Local only (no remote) | raw local file content | `(local only)\n` marker | 0 |
| Remote only (no local) | raw remote file content | `(remote only)\n` marker | 0 |

A binary file produces no body — `unified` / `raw` payloads are skipped and the side marker (or no output, when both sides are equal) is the only signal.

#### `--json` output (opt-in, LLM-friendly)

```sh
gitless-sync diff <relative/path> --repo owner/name --local . --json
```

Emits one stdout JSON line and writes nothing to stderr (no side marker). The shape is uniform across cases — callers parse one schema instead of branching on text-vs-empty-vs-stderr-marker:

```json
{"side": "both" | "local_only" | "remote_only", "unified": string | null, "raw": string | null, "binary": bool}
```

Examples:

```text
# both sides, normalize-equal
{"side":"both","unified":"","raw":null,"binary":false}

# local only
{"side":"local_only","unified":null,"raw":"<file content>","binary":false}

# both sides, normalize-diff
{"side":"both","unified":"--- a/...\n+++ b/...\n@@ ...","raw":null,"binary":false}
```

Authoritative schema: `docs/specs/spec-cli-interface.md` § diff --json 출력 형식 + `docs/specs/spec-output-schema.md` § diff sub-schema.

## Exit codes

| Code | Meaning |
|------|---------|
| 0 | Normal (drift may exist; the result is still considered success) |
| 1 | User input / config error, including `gh` not installed |
| 2 | Authentication failed (`gh: Bad credentials`) |
| 3 | GitHub API rate limit hit, or other non-classified `gh` error |
| 4 | Partial failure (result emitted, some files marked `failed`) |
| 5 | Trees API truncated — repo too large for v0.1 |

stdout is reserved for the result JSON. All progress, warnings, and errors go to stderr as one-line JSON: `{"error_code": "...", "message": "...", "context": {}}`. See `docs/specs/spec-error-contracts.md`.

## Troubleshooting

### `gh` not on PATH

If `gh` is missing, the binary fails fast on the first API call. Empirical verification on Windows 11 (2026-05-06, debug build):

```text
$ PATH stripped to %WINDIR%\System32 only
$ gitless-sync.exe scan --repo foo/bar --local .
stderr> {"error_code":"CONFIG_ERROR","message":"Configuration error: gh CLI not found in PATH; install from https://cli.github.com/"}
exit  > 1
```

The hardcoded message lives at `crates/gitless-sync/src/shared/gh.rs::GH_NOT_FOUND_MESSAGE` and is pinned by the unit test `gh_not_found_message_contains_install_hint`.

### Authentication failed

Run `gh auth status` to confirm the active account, then `gh auth login` (or `gh auth refresh`) if the token has expired.

### `Trees truncated`

The repo exceeds GitHub's Trees API single-response limit (≈7 MB or ≈100 k entries). v0.1 does not split the request — see guardrail G-002.

## References

- `docs/adr/0001-gh-subprocess-and-drop-push-tool.md` — read-only stance and `gh` subprocess decision.
- `docs/adr/0002-migrate-v0.1-to-gh-subprocess.md` — v0.1 migration of REST calls from `ureq` to `gh api`.
- `docs/specs/spec-cli-interface.md` — full CLI surface.
- `docs/specs/spec-output-schema.md` — output JSON shape.
- `docs/specs/spec-error-contracts.md` — error codes, exit codes, and `gh` stderr mapping.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
