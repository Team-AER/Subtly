# macOS Code Signing Setup Guide

## Build Log Analysis

### From Your Latest CI Run:

**❌ macOS (CRITICAL - Your Corrupted App Issue):**
```
• skipped macOS application code signing
  reason=cannot find valid "Developer ID Application" identity
  0 identities found - 0 valid identities found
```
This is **exactly** why macOS says your app is corrupted!

**✅ Linux:** Built successfully (AppImage + deb)  
**✅ Windows:** Built successfully (NSIS installer)

---

## The Problem
macOS blocks unsigned apps downloaded from the internet with a "corrupted" error. This is Gatekeeper protection.

## Quick Solutions

### For Testing Only (Temporary)
Users can bypass the error locally:
```bash
xattr -cr /Applications/Subtly.app
```

### For Distribution (Proper Solution)

## Step 1: Get an Apple Developer Account
1. Sign up at [developer.apple.com](https://developer.apple.com) ($99/year)
2. Create a **Developer ID Application** certificate in Xcode or the Apple Developer portal

## Step 2: Export Your Certificate
```bash
# In Keychain Access, export your "Developer ID Application" certificate
# File -> Export Items -> Save as .p12 file with a password
```

## Step 3: Convert Certificate to Base64
```bash
base64 -i certificate.p12 | pbcopy
# This copies the base64 string to your clipboard
```

## Step 4: Add GitHub Secrets
In your GitHub repository settings, add these secrets:

### Required for Code Signing:
- `CSC_LINK`: The base64-encoded .p12 certificate (from Step 3)
- `CSC_KEY_PASSWORD`: The password you used when exporting the .p12

### Optional for Notarization (Recommended):
- `APPLE_ID`: Your Apple ID email
- `APPLE_APP_SPECIFIC_PASSWORD`: App-specific password (create at appleid.apple.com)
- `APPLE_TEAM_ID`: Your 10-character Team ID (find in Apple Developer portal)

## Step 5: Test Locally

### Build with signing:
```bash
export CSC_LINK=/path/to/certificate.p12
export CSC_KEY_PASSWORD=your-password

pnpm build:runtime
pnpm build
pnpm pack
```

### Build without signing (for local dev):
```bash
export CSC_IDENTITY_AUTO_DISCOVERY=false
pnpm pack
```

## How It Works

When the GitHub secrets are set, electron-builder will:
1. ✅ Sign the app with your Developer ID certificate
2. ✅ Enable Hardened Runtime
3. ✅ Apply entitlements (JIT, audio access, etc.)
4. ✅ Notarize with Apple (if notarization secrets are set)

When secrets are NOT set, it will:
- ⚠️ Build an unsigned app (for development only)
- Users will need to use `xattr -cr` to bypass Gatekeeper

## Verification

After building with signing, verify:
```bash
# Check code signature
codesign -dvv release/mac-arm64/Subtly.app

# Check if notarized (if you set up notarization)
spctl -a -vv -t install release/mac-arm64/Subtly.app
```

## Alternative: Ad-hoc Signing for Local Testing

If you don't have an Apple Developer account, you can use ad-hoc signing locally:

```bash
# Build unsigned
export CSC_IDENTITY_AUTO_DISCOVERY=false
pnpm pack

# Ad-hoc sign (won't work for distribution, only local testing)
codesign --force --deep --sign - release/mac-arm64/Subtly.app
```

## Resources
- [electron-builder Code Signing](https://www.electron.build/code-signing)
- [Apple Notarization Guide](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
