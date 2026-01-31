#!/bin/bash
# Script to export and encode your Apple Developer ID certificate for CI/CD

echo "🔐 Apple Developer ID Certificate Export Helper"
echo "================================================"
echo ""

# Step 1: Find the certificate
echo "Step 1: Finding your Developer ID Application certificate..."
echo ""
security find-identity -v -p codesigning | grep "Developer ID Application"
echo ""
echo "Copy the certificate name from above (the part in quotes after the hash)"
read -p "Press Enter to continue..."

# Step 2: Export instructions
echo ""
echo "Step 2: Export the certificate from Keychain Access"
echo "================================================"
echo ""
echo "1. Open 'Keychain Access' application"
echo "2. In the left sidebar, select 'login' keychain"
echo "3. In the category list, select 'My Certificates'"
echo "4. Find your 'Developer ID Application' certificate"
echo "5. Right-click on it and select 'Export...'"
echo "6. Save as: certificate.p12"
echo "7. Choose a password (you'll need this for CSC_KEY_PASSWORD)"
echo "   Make it strong but memorable - you'll use it in GitHub secrets"
echo ""
read -p "Enter the password you chose: " cert_password
echo ""

# Wait for export
read -p "Have you exported certificate.p12? (Press Enter when done)..." 

# Step 3: Base64 encode
echo ""
echo "Step 3: Converting certificate to base64..."
echo ""

if [ -f "$HOME/Desktop/certificate.p12" ]; then
    CERT_PATH="$HOME/Desktop/certificate.p12"
elif [ -f "$HOME/Downloads/certificate.p12" ]; then
    CERT_PATH="$HOME/Downloads/certificate.p12"
else
    echo "⚠️  Certificate not found in Desktop or Downloads"
    echo "Please enter the full path to certificate.p12:"
    read -p "Path: " CERT_PATH
fi

if [ ! -f "$CERT_PATH" ]; then
    echo "❌ Error: Certificate file not found at $CERT_PATH"
    exit 1
fi

# Create base64 encoded version
base64 -i "$CERT_PATH" -o "$HOME/Desktop/certificate-base64.txt"

echo ""
echo "✅ Success! Base64-encoded certificate created!"
echo ""
echo "📋 Next Steps for GitHub Secrets:"
echo "================================================"
echo ""
echo "1. Go to: https://github.com/Team-AER/Subtly/settings/secrets/actions"
echo ""
echo "2. Update or create these secrets:"
echo ""
echo "   Secret Name: CSC_LINK"
echo "   Secret Value: Copy the entire contents of:"
echo "   👉 $HOME/Desktop/certificate-base64.txt"
echo ""
echo "   Secret Name: CSC_KEY_PASSWORD"
echo "   Secret Value: $cert_password"
echo ""
echo "3. Keep these existing secrets (already configured):"
echo "   - APPLE_ID"
echo "   - APPLE_APP_SPECIFIC_PASSWORD"
echo "   - APPLE_TEAM_ID"
echo ""
echo "================================================"
echo ""
echo "⚠️  IMPORTANT SECURITY NOTES:"
echo "1. Delete certificate.p12 and certificate-base64.txt after uploading to GitHub"
echo "2. Never commit these files to git"
echo "3. Store the password in a secure password manager"
echo ""
read -p "Press Enter to open the base64 file for copying..."

# Open the file
if command -v open &> /dev/null; then
    open "$HOME/Desktop/certificate-base64.txt"
elif command -v xdg-open &> /dev/null; then
    xdg-open "$HOME/Desktop/certificate-base64.txt"
fi

echo ""
echo "Done! Remember to delete the certificate files after setup."
