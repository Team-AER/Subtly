# macOS signing and notarization

Subtly v2 uses cargo-packager and the Rust `xtask notarize` helper. There are no Electron build commands or JavaScript notarization hooks in the current application. This guide describes the checked-in [Build workflow](../.github/workflows/build.yml); it does not assert that repository secrets or a particular release are signed.

## Local packaging

Use a Developer ID Application certificate and its private key installed in your macOS keychain, plus Xcode command-line tools. Select the matching identity without exposing the private key:

```sh
security find-identity -v -p codesigning
cargo install cargo-packager --locked
cargo run -p xtask -- download-assets
cargo build --release -p subtly-ui
APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
  cargo packager --release --formats app
```

Check the actual app path under `release/`; packaging can vary with target/version. For the commands below, set `SUBTLY_APP` to that generated bundle:

```sh
SUBTLY_APP="release/Subtly.app"
codesign --verify --deep --strict --verbose=2 "$SUBTLY_APP"
cargo run -p xtask -- notarize "$SUBTLY_APP"
xcrun stapler validate "$SUBTLY_APP"
spctl --assess --type execute --verbose=2 "$SUBTLY_APP"
```

Before running `notarize`, provide `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD` and `APPLE_TEAM_ID` through your credential manager or protected environment. Do not paste secrets into repository files, shell history, documentation or logs.

The helper zips a raw `.app` with `ditto`, submits it using `xcrun notarytool --wait`, requires Apple's `Accepted` status and staples the original bundle. It skips on non-macOS hosts or when any required credential is missing, so a successful command exit alone does not prove notarization. Use the validation commands and inspect the helper's result. Although the helper can submit a `.zip`, ZIP files cannot receive a stapled ticket; prefer `.app` or `.dmg` inputs.

## GitHub Actions credentials

The macOS job imports a certificate into a temporary keychain and exports its signing identity. Configure these repository Actions secrets through GitHub's settings:

| Secret | Role |
|---|---|
| `CSC_LINK` | Exported `.p12` certificate and private key, normally base64-encoded for CI |
| `CSC_KEY_PASSWORD` | Password used to protect that `.p12` export |
| `APPLE_SIGNING_IDENTITY` | Optional explicit identity; the workflow can derive it from the imported certificate |
| `APPLE_ID` | Apple account used for notarization |
| `APPLE_APP_SPECIFIC_PASSWORD` | Account's app-specific notarization password |
| `APPLE_TEAM_ID` | Developer team identifier |

An explicit identity is only useful if its certificate and private key are accessible in the build keychain. Missing credentials can produce unsigned artifacts or skipped notarization, as described by workflow warnings. Read the run's signing and notarization results rather than treating uploaded artifacts as proof.

The workflow packages an `.app`, signs/notarizes it, then creates the `.dmg` with `hdiutil` and notarizes/staples the DMG. It avoids asking cargo-packager to rebuild the app during DMG creation, which could discard the signature.

## Troubleshooting

- **No valid identity:** confirm that the certificate includes its private key, is valid, and is available in the active keychain. Check `security find-identity -v -p codesigning`.
- **Certificate import fails:** confirm the `.p12` export, its password and the encoding of `CSC_LINK`; inspect the workflow import error without printing the secret.
- **Notarization rejected:** the Rust helper retrieves Apple's submission log. Inspect the specific rejected file or signature and rebuild/re-sign before retrying.
- **Gatekeeper rejects the app:** check its signature, Apple's acceptance and the stapled ticket. A damaged-app message alone does not identify the cause; do not replace verification with quarantine removal.

For a quick certificate checklist see [certificate-setup-quick-guide.md](certificate-setup-quick-guide.md). The older [MACOS_SIGNING.md](MACOS_SIGNING.md) filename remains as a pointer to this current guide.
