# Contributing to NimblePost

Bug reports, documentation improvements, and focused pull requests are
welcome. Please follow the [code of conduct](CODE_OF_CONDUCT.md).

## Open an issue

Search [existing issues](https://github.com/Khr0x/nimblepost/issues) before
[opening an issue](https://github.com/Khr0x/nimblepost/issues/new/choose).
For bugs, include the NimblePost version, operating system, reproduction
steps, and expected and actual behavior. Use a minimal example with
synthetic data and remove credentials and private API data from attachments.
Discuss larger changes in an issue before implementing them.

Use the private channel documented in the [security policy](SECURITY.md)
to report a vulnerability.

## Submit a pull request

1. Fork the repository and create a branch from `main`.
2. Follow the [local setup](README.md#run-the-desktop-app) and keep the change
   focused. Put shared execution behavior in `crates/core` so Desktop and
   CLI stay consistent.
3. Add or update tests when behavior changes. Run the
   [development checks](README.md#development-checks) and use synthetic data
   or local servers for test cases.
4. Add a Changeset for user-facing behavior changes. See the
   [release guide](.changeset/README.md).
5. Explain what changed and how you verified it. Link the related issue
   if there is one.

Contributions are accepted under the project's [Apache-2.0 license](LICENSE).
Third-party material retains its upstream license and attribution.
