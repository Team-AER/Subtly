# Quick Guide: Setting Up Code Signing for GitHub Actions

This is a quick reference guide for setting up the `CSC_LINK` and `CSC_KEY_PASSWORD` secrets.

## ⚡ Quick Steps

### 1️⃣ Export Certificate (On Your Mac)

```bash
# Open Keychain Access
open "/Applications/Utilities/Keychain Access.app"
```

1. **Login** keychain → **My Certificates**
2. Find: **Developer ID Application: Prakhar Shukla (TEAMID)**
3. Right-click → **Export "Developer ID Application..."**
4. Save as: `certificate.p12`
5. **Set a password** (e.g., `YourSecurePassword123`)

### 2️⃣ Convert to Base64

```bash
# Convert the certificate
base64 -i ~/Desktop/certificate.p12 -o ~/Desktop/certificate-base64.txt

# View the file (to copy)
cat ~/Desktop/certificate-base64.txt
```

### 3️⃣ Update GitHub Secrets

Go to: https://github.com/Team-AER/Subtly/settings/secrets/actions

**Update these two secrets:**

| Secret Name | Value |
|-------------|-------|
| `CSC_LINK` | Paste the **entire contents** of `certificate-base64.txt` |
| `CSC_KEY_PASSWORD` | The password you set in step 1 (e.g., `YourSecurePassword123`) |

### 4️⃣ Clean Up

```bash
# Delete the certificate files (IMPORTANT for security!)
rm ~/Desktop/certificate.p12
rm ~/Desktop/certificate-base64.txt
```

### 5️⃣ Test

Push a commit and check the GitHub Actions build.

---

## ✅ Verification Checklist

Before triggering a build, verify you have these 5 secrets configured:

- [ ] `CSC_LINK` - Base64-encoded certificate (very long string)
- [ ] `CSC_KEY_PASSWORD` - Certificate password
- [ ] `APPLE_ID` - Your Apple ID email
- [ ] `APPLE_APP_SPECIFIC_PASSWORD` - App-specific password
- [ ] `APPLE_TEAM_ID` - 10-character Team ID

## 🔍 Troubleshooting

### Error: "empty password will be used"
→ `CSC_KEY_PASSWORD` is not set or is empty

### Error: "not a file" or "CSC_LINK is not a valid file"
→ `CSC_LINK` contains invalid base64 or a file path instead of base64-encoded content

### Error: "Failed to decode"
→ The base64 encoding is corrupted. Re-export and re-encode the certificate

## 📚 More Info

See [`docs/code-signing.md`](./code-signing.md) for comprehensive documentation.
