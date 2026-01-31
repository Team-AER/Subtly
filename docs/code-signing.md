# macOS Code Signing and Notarization

This guide explains how to set up code signing and notarization for Subtly on macOS to ensure users can open the app without Gatekeeper security warnings.

## Overview

macOS requires apps distributed outside the App Store to be:
1. **Signed** with a Developer ID certificate
2. **Notarized** by Apple (uploaded to Apple for automated security scanning)
3. **Stapled** with the notarization ticket

Without notarization, users see: "Apple could not verify 'Subtly.app' is free of malware."

## Requirements

### Apple Developer Account
- **Paid Apple Developer Program membership** ($99/year)
- Enroll at: https://developer.apple.com/programs/

### Developer ID Certificate
1. Log in to [Apple Developer](https://developer.apple.com/account/)
2. Go to Certificates, Identifiers & Profiles
3. Create a **Developer ID Application** certificate
4. Download and install in Keychain
5. Export as `.p12` file for CI/CD use

### App-Specific Password
1. Go to [appleid.apple.com](https://appleid.apple.com/)
2. Sign in with your Apple ID
3. Navigate to Security → App-Specific Passwords
4. Generate a new password (e.g., "Subtly Notarization")
5. Save this password securely

### Team ID
1. Go to [Apple Developer Membership](https://developer.apple.com/account/#/membership/)
2. Find your **Team ID** (10-character identifier)
3. Save this for later use

## Configuration

Subtly uses electron-builder's `afterSign` hook to handle notarization. This is implemented in [`scripts/notarize.js`](../scripts/notarize.js).

The notarization script automatically runs after signing and will:
- ✅ Notarize the app if all required environment variables are present
- ⏭️ Skip notarization if any credentials are missing (allows local development builds)
- ❌ Fail the build if credentials are present but invalid

### Environment Variables

Subtly uses the following environment variables for code signing and notarization:

| Variable | Description | Where to Get It |
|----------|-------------|-----------------|
| `CSC_LINK` | Base64-encoded `.p12` certificate or path to `.p12` file | Export from Keychain |
| `CSC_KEY_PASSWORD` | Password for the `.p12` certificate | Set when exporting |
| `APPLE_ID` | Your Apple ID email | Your Apple account email |
| `APPLE_APP_SPECIFIC_PASSWORD` | App-specific password | Generated at appleid.apple.com |
| `APPLE_TEAM_ID` | Your Apple Developer Team ID | Found in developer account |

### Local Development

For local builds with notarization:

```bash
# Set up environment variables (add to ~/.zshrc or ~/.bashrc for persistence)
export APPLE_ID="your-email@example.com"
export APPLE_APP_SPECIFIC_PASSWORD="abcd-efgh-ijkl-mnop"
export APPLE_TEAM_ID="XXXXXXXXXX"

# Optional: For signing (if not using certificate from Keychain)
export CSC_LINK="/path/to/certificate.p12"
export CSC_KEY_PASSWORD="your-p12-password"

# Build and package
pnpm build:runtime
pnpm build
pnpm pack
```

> **Note:** If you have the Developer ID certificate in your Keychain and it's valid, electron-builder will automatically find and use it. You only need `CSC_LINK` if you want to specify a specific certificate file.

### CI/CD (GitHub Actions)

The GitHub Actions workflow is already configured. You need to add these secrets:

1. Go to your GitHub repository
2. Navigate to **Settings → Secrets and variables → Actions**
3. Add the following **Repository secrets**:

   - `CSC_LINK`: Base64-encoded `.p12` certificate
     ```bash
     # Create base64-encoded certificate
     cat certificate.p12 | base64 > certificate.base64.txt
     # Copy contents and paste as secret value
     ```
   - `CSC_KEY_PASSWORD`: Password for the `.p12` file
   - `APPLE_ID`: Your Apple ID email
   - `APPLE_APP_SPECIFIC_PASSWORD`: App-specific password from appleid.apple.com
   - `APPLE_TEAM_ID`: Your 10-character Team ID

## Building Without Notarization

If you want to build locally without notarization (e.g., for development):

```bash
# Simply don't set the APPLE_* environment variables
pnpm build:runtime
pnpm build
pnpm pack
```

The build will succeed, but the app won't be notarized. You can still open it locally by right-clicking and selecting "Open" (bypasses Gatekeeper for the first launch).

## Verification

### Check Code Signature

```bash
# Verify the app is signed
codesign -vvv --deep --strict release/mac-arm64/Subtly.app

# Should output:
# release/mac-arm64/Subtly.app: valid on disk
# release/mac-arm64/Subtly.app: satisfies its Designated Requirement
```

### Check Notarization Status

```bash
# Check if app is notarized and Gatekeeper will accept it
spctl -a -vv -t install release/mac-arm64/Subtly.app

# Should output something like:
# release/mac-arm64/Subtly.app: accepted
# source=Notarized Developer ID
```

### Check Stapling

```bash
# Check if notarization ticket is stapled to the app
stapler validate release/mac-arm64/Subtly.app

# Should output:
# The validate action worked!
```

## Troubleshooting

### "No Developer ID certificate found"

**Solution:** Install your Developer ID Application certificate in Keychain Access, or set `CSC_LINK` to point to your `.p12` file.

### "Notarization failed"

**Possible causes:**
- Invalid Apple ID or app-specific password
- Incorrect Team ID
- App not properly signed before notarization
- Missing entitlements

**Solution:** Check the build logs for specific error messages from Apple's notarization service.

### "App is damaged and can't be opened"

This usually means the app signature is invalid.

**Solution:**
```bash
# Remove quarantine attribute
xattr -cr release/mac-arm64/Subtly.app
```

### Notarization Takes Too Long

Notarization typically takes 1-15 minutes. If it takes longer:
- Check Apple's system status: https://developer.apple.com/system-status/
- Wait and retry later if Apple's services are experiencing issues

### "Unexpected token 'E', Error: int... is not valid JSON"

This error occurs when the notarization tool receives an error message instead of JSON output.

**Common causes:**
- Missing or invalid environment variables (`APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID`)
- Incorrect Team ID format (should be 10-character alphanumeric)
- Invalid app-specific password
- Environment variables not properly exported in CI/CD

**Solution:**
1. Verify all three environment variables are set:
   ```bash
   echo $APPLE_ID
   echo $APPLE_APP_SPECIFIC_PASSWORD  
   echo $APPLE_TEAM_ID
   ```
2. Ensure Team ID is exactly 10 characters (no quotes, no spaces)
3. Regenerate app-specific password if needed
4. In CI/CD, verify secrets are properly configured in GitHub Actions

## References

- [Apple Developer Documentation: Notarizing macOS Software](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [electron-builder Code Signing](https://www.electron.build/code-signing)
- [electron-builder macOS Configuration](https://www.electron.build/configuration/mac)
- [@electron/notarize](https://github.com/electron/notarize)

## Security Best Practices

1. **Never commit certificates or passwords** to version control
2. **Use environment variables** for sensitive credentials
3. **Rotate app-specific passwords** periodically
4. **Use GitHub repository secrets** for CI/CD credentials
5. **Protect your `.p12` file** - store it securely encrypted
6. **Enable 2FA** on your Apple ID account
