const { notarize } = require('@electron/notarize');

/**
 * electron-builder afterSign hook for macOS notarization
 * 
 * This is called automatically after the app is signed.
 * It will notarize the app if all required environment variables are present.
 * 
 * Required environment variables:
 * - APPLE_ID: Your Apple ID email
 * - APPLE_APP_SPECIFIC_PASSWORD: App-specific password from appleid.apple.com
 * - APPLE_TEAM_ID: Your 10-character Apple Developer Team ID
 */
exports.default = async function notarizeApp(context) {
    const { electronPlatformName, appOutDir } = context;

    // Only notarize macOS builds
    if (electronPlatformName !== 'darwin') {
        console.log('  • Skipping notarization (not macOS)');
        return;
    }

    // Check if all required environment variables are present
    const appleId = process.env.APPLE_ID;
    const appleIdPassword = process.env.APPLE_APP_SPECIFIC_PASSWORD;
    const teamId = process.env.APPLE_TEAM_ID;

    if (!appleId || !appleIdPassword || !teamId) {
        console.log('  • Skipping notarization (missing credentials)');
        console.log(`    APPLE_ID: ${appleId ? '✓' : '✗'}`);
        console.log(`    APPLE_APP_SPECIFIC_PASSWORD: ${appleIdPassword ? '✓' : '✗'}`);
        console.log(`    APPLE_TEAM_ID: ${teamId ? '✓' : '✗'}`);
        return;
    }

    const appName = context.packager.appInfo.productFilename;
    const appPath = `${appOutDir}/${appName}.app`;
    const appBundleId = context.packager.appInfo.id;

    console.log(`  • Notarizing ${appName}.app`);
    console.log(`    App path: ${appPath}`);
    console.log(`    App Bundle ID: ${appBundleId}`);
    console.log(`    Apple ID: ${appleId}`);
    console.log(`    Team ID: ${teamId}`);

    try {
        await notarize({
            appPath,
            appBundleId,
            appleId,
            appleIdPassword,
            teamId,
        });
        console.log(`  ✓ Successfully notarized ${appName}.app`);
    } catch (error) {
        console.error(`  ✗ Notarization failed: ${error.message}`);
        throw error;
    }
};
