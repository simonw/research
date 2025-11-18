#!/bin/bash
set -e

echo "=========================================="
echo "Installing APKs from /tmp directory"
echo "=========================================="

# Wait for device
adb wait-for-device
sleep 2

# Enable root
adb root
sleep 2

# Install all APKs in /tmp
echo "Looking for APK files in /tmp..."
APK_COUNT=$(adb shell "ls /tmp/*.apk 2>/dev/null | wc -l" | tr -d ' ')

if [ "$APK_COUNT" -eq 0 ]; then
    echo "No APK files found in /tmp"
    echo ""
    echo "To install APKs manually:"
    echo "1. Download APKs to your local machine"
    echo "2. Transfer to VM: gcloud compute scp --zone=us-central1-a --project=corsali-development <apk-file> android-mitm-mvp:/tmp/"
    echo "3. Copy to container: gcloud compute ssh android-mitm-mvp --zone=us-central1-a --project=corsali-development --command='docker cp <apk-file> dockerify-android-mitm:/tmp/'"
    echo "4. Run this script again"
    exit 1
fi

echo "Found $APK_COUNT APK file(s)"
echo ""

# List APKs
adb shell "ls -lh /tmp/*.apk" | grep -v "No such file"

# Install each APK
for apk in $(adb shell "ls /tmp/*.apk 2>/dev/null"); do
    apk=$(echo "$apk" | tr -d '\r\n')
    apk_name=$(basename "$apk")
    echo ""
    echo "Installing $apk_name..."
    adb install -r -g "$apk" 2>&1 | grep -E "(Success|Failure|Error|INSTALL)" || echo "  Installation attempted"
done

echo ""
echo "=========================================="
echo "Installation complete!"
echo "=========================================="
echo "Installed packages:"
adb shell pm list packages | grep -E "(youtube|newpipe|google|vending)" || echo "  (check manually with: adb shell pm list packages)"
echo "=========================================="

