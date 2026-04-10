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
