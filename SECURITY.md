# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Oxigit, please report it responsibly.

**Email:** [security@b-nova.com](mailto:security@b-nova.com)

Please include:

- A description of the vulnerability
- Steps to reproduce
- Affected versions (if known)
- Any potential impact assessment

## What to Expect

- **Acknowledgement** within 48 hours
- **Status update** within 7 days
- We will coordinate disclosure timing with you

## Encryption at Rest

Oxigit encrypts sensitive data before storing it in the database.

### What is encrypted

- User LLM API keys (`user_settings.llm_api_key`)

### How it works

- **Cipher:** AES-256-GCM (NIST SP 800-38D), an authenticated encryption with associated data (AEAD) scheme
- **Key derivation:** HKDF-SHA256 derives a purpose-specific encryption key from the master secret (`data_dir/secret_key`), using the info string `oxigit-api-key-encryption`
- **Nonce:** A fresh random 96-bit nonce is generated for every encryption operation
- **Storage format:** `enc:<base64(nonce || ciphertext || tag)>` in the existing TEXT column — no schema migration required
- **Legacy handling:** Values without the `enc:` prefix are treated as plaintext and migrated automatically on startup
- **UI masking:** Decrypted keys are never sent to the browser; the settings page displays a masked form (e.g. `sk-p...xY7z`)

### Master secret

The 32-byte master secret is stored at `{data_dir}/secret_key` and is generated automatically on first startup. This key is used for both session signing (HMAC-SHA256) and, via HKDF, encryption at rest.

**If the secret key file is lost or regenerated, all encrypted API keys become unrecoverable.** Back up this file alongside your database.

## Scope

The following are in scope:

- Authentication and authorization bypasses
- Data exposure or leakage
- Remote code execution
- Path traversal in git transport (HTTP or SSH)
- Cross-site scripting (XSS) or injection attacks
- Session handling issues

The following are out of scope:

- Denial of service attacks
- Social engineering
- Issues in dependencies (please report these upstream)
- Self-hosted instances with intentionally weakened configuration

## Supported Versions

Security fixes are applied to the latest release. We do not backport fixes to older versions.

## Disclosure Policy

We follow coordinated disclosure. We ask that you give us reasonable time to address the issue before public disclosure.
