# Certificate setup checklist

Subtly's macOS GitHub Actions job uses cargo-packager and `xtask`, not electron-builder. Full packaging and verification steps are in [code-signing.md](code-signing.md).

1. In Keychain Access, find a valid **Developer ID Application** certificate under My Certificates and verify it has its private key.
2. Export the certificate and key as a password-protected `.p12`. Keep the export in a protected temporary location.
3. Supply its base64 encoding to the repository's `CSC_LINK` Actions secret and its export password to `CSC_KEY_PASSWORD`, using GitHub's secret settings or your secure tooling. Do not print certificate contents into logs.
4. Configure `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD` and `APPLE_TEAM_ID` for notarization. `APPLE_SIGNING_IDENTITY` is optional when the workflow derives the identity from the imported certificate.
5. Remove temporary certificate exports when the secure upload is complete. Inspect a subsequent macOS build's certificate-import, signing and notarization results, then validate its actual app/DMG artifact.

This checklist documents the required configuration; it does not imply any of these secrets are currently present or valid. The existing `scripts/export-certificate.sh` is a manual helper, not part of the Rust build; review its output handling before use.
