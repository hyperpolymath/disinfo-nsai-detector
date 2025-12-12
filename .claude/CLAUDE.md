# Project Instructions

## ⚠️ CONVERSION REQUIRED: Go → Rust

This repository needs full conversion from Go to Rust.

### Policy (RSR)
- **REQUIRED**: Rust for systems code
- **FORBIDDEN**: Go (.go files)
- **EXCEPTION**: None for this repo

### When modifying this codebase:
1. DO NOT add new Go files
2. When touching existing Go, convert to Rust
3. Use equivalent Rust crates (async-nats, ort, prometheus, prost)
4. Delete the original Go file after conversion

See RUST_CONVERSION_NEEDED.md for detailed migration guide.

## Language & Security Policy (RSR)

### Allowed Languages (Primary → Fallback)
- **Systems/ML**: Rust
- **Web/Scripts**: ReScript → TypeScript (legacy only)
- **TUI**: Ada/SPARK
- **WordPress**: PHP (with security CI)
- **LSP**: Java (exception for IDE compatibility)

### Banned Languages
- Python (except SaltStack)
- Ruby (use Rust/Ada/Crystal)
- Perl (use Rust)
- New Java/Kotlin (except LSP)

### Package Management
- **Primary**: Guix (guix.scm)
- **Fallback**: Nix (flake.nix)

### Security Requirements
- No MD5/SHA1 for security (use SHA256+)
- HTTPS only (no HTTP URLs)
- No hardcoded secrets
