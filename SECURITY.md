# Security policy

NimblePost is ALPHA software. Its APIs, file handling and distribution are still
being stabilized. Security fixes are delivered in new releases.

## Supported versions

| Version | Security maintenance |
| --- | --- |
| Latest published ALPHA | Fixes delivered in a subsequent release |
| Older ALPHA releases | No backports; upgrade to the latest release |

Reports affecting the default branch are also welcome. A branch build is not a
supported release. Published versions are listed in
[GitHub Releases](https://github.com/Khr0x/nimblepost/releases).

## Reporting a vulnerability

Use [GitHub private vulnerability reporting](https://github.com/Khr0x/nimblepost/security/advisories/new)
(**Security → Report a vulnerability**) to contact the maintainers privately.
Do not disclose vulnerabilities, exploits or sensitive data in public issues,
discussions or pull requests before a coordinated disclosure.

Include, when available:

- The affected version or commit, OS and architecture.
- Reproduction steps and a minimal example using synthetic data.
- The expected behavior, observed behavior and potential security impact.
- Any suggested mitigation or fix.

Remove real credentials, tokens, private collection data and personal information
from attachments and logs. Test only systems and data you own or are authorized
to assess. A minimal local reproduction is sufficient; do not access other
users' data or disrupt services to demonstrate impact.

## Handling and disclosure

Maintainers will assess the report, ask for missing details and coordinate a
fix and disclosure with the reporter through the private advisory. Where
appropriate, a security advisory will identify affected versions, mitigations
and the release containing the fix. Keep exploit details private while that
coordination is in progress.

This policy does not promise fixed response or resolution times, or payments
for vulnerability reports.

For ordinary bugs and feature requests without security implications, use
[GitHub Issues](https://github.com/Khr0x/nimblepost/issues).
