# Security Policy

## Reporting a vulnerability

If you believe you have found a security vulnerability in Dakera, please report it
privately. **Do not open a public GitHub issue for security reports.**

Contact us via our [LinkedIn page](https://linkedin.com/company/dakera-ai) and
include "Security" in your message so we can route it appropriately. We aim to
acknowledge reports within 5 business days.

Please include, where possible: affected version/package, a description of the
issue, reproduction steps, and any relevant logs or proof-of-concept.

## Supported versions

Security fixes are applied to the latest released version. We recommend always
running the most recent release. Dakera is currently on the `0.11.x` line (public
alpha).

## Data handling

- The self-hosted Dakera engine processes all memory data on your own
  infrastructure; memory contents and personal data are never transmitted to Dakera.
- Encryption at rest is available via AES-256-GCM (`DAKERA_ENCRYPTION_KEY`).
- The engine sends only anonymous operational telemetry (version, OS family,
  deployment type); disable with `DAKERA_TELEMETRY=off` or `DO_NOT_TRACK=1`.

## Third-party assessments

None to date. This section will be updated if and when an independent security
assessment is completed.
