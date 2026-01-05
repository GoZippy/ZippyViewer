# TestFlight Configuration for ZippyRemote

## Overview

TestFlight allows beta testing of ZippyRemote before App Store release. This document outlines the TestFlight setup and testing process.

## Prerequisites

1. **Apple Developer Account**
   - Enrolled in Apple Developer Program ($99/year)
   - App ID created: `io.zippyremote.app`
   - Provisioning profiles configured

2. **Xcode Setup**
   - Xcode 14.0 or later
   - Valid signing certificates
   - App Store Connect API access (optional, for automation)

## TestFlight Setup Steps

### 1. Create App in App Store Connect

1. Log in to [App Store Connect](https://appstoreconnect.apple.com)
2. Navigate to "My Apps"
3. Click "+" to create new app
4. Fill in:
   - **Platform**: iOS
   - **Name**: ZippyRemote
   - **Primary Language**: English
   - **Bundle ID**: io.zippyremote.app
   - **SKU**: zippyremote-ios-001
   - **User Access**: Full Access

### 2. Configure App Information

In App Store Connect, configure:
- App Information (name, category, age rating)
- Pricing and Availability
- App Privacy (data collection, tracking)
- App Store Preview (screenshots, description)

### 3. Upload Build to TestFlight

#### Using Xcode

1. Open `ios-app/ZippyRemote.xcodeproj` in Xcode
2. Select "Any iOS Device" as destination
3. Product → Archive
4. In Organizer window:
   - Click "Distribute App"
   - Select "App Store Connect"
   - Select "Upload"
   - Choose automatic signing
   - Click "Upload"
5. Wait for processing (15-30 minutes)

#### Using Command Line

```bash
# Build and archive
xcodebuild -workspace ios-app/ZippyRemote.xcworkspace \
  -scheme ZippyRemote \
  -configuration Release \
  -archivePath build/ZippyRemote.xcarchive \
  archive

# Upload to App Store Connect
xcrun altool --upload-app \
  --type ios \
  --file build/ZippyRemote.ipa \
  --apiKey YOUR_API_KEY \
  --apiIssuer YOUR_ISSUER_ID
```

### 4. Configure TestFlight

1. In App Store Connect, go to TestFlight tab
2. Wait for build processing to complete
3. Add build to TestFlight testing

### 5. Internal Testing

#### Add Internal Testers

1. Go to TestFlight → Internal Testing
2. Click "+" to add internal testers
3. Add team members (up to 100)
4. Internal testers can test immediately after build is processed

#### Internal Testing Groups

Create groups for different testing scenarios:
- **Core Team**: Developers and QA
- **Beta Testers**: Selected external testers
- **Stakeholders**: Management and product team

### 6. External Testing

#### Add External Testers

1. Go to TestFlight → External Testing
2. Create new group (e.g., "Beta Testers")
3. Add build to group
4. Submit for Beta App Review (required for external testing)
5. Add testers via email or public link

#### Beta App Review Requirements

- App must comply with App Store Review Guidelines
- Provide demo account if needed
- Explain any incomplete features
- Provide contact information

### 7. TestFlight Build Configuration

#### Build Settings

In Xcode project settings:
- **Version**: 1.0
- **Build**: Increment for each upload (1, 2, 3, ...)
- **Bundle Identifier**: io.zippyremote.app
- **Signing**: Automatic or manual provisioning

#### Info.plist Configuration

Ensure Info.plist includes:
- Camera usage description (for QR scanning)
- Photo library usage description
- Network usage description
- Background modes (if applicable)

### 8. Testing Checklist

#### Pre-Upload Checklist

- [ ] App compiles without errors
- [ ] All required permissions declared in Info.plist
- [ ] App icon and launch screen configured
- [ ] Version and build numbers incremented
- [ ] Code signing configured correctly
- [ ] No debug logging in production build
- [ ] Crash reporting configured (optional)

#### TestFlight Testing Checklist

- [ ] App installs successfully
- [ ] App launches without crashes
- [ ] All core features work
- [ ] Network connectivity works
- [ ] Camera permission works (QR scanning)
- [ ] Touch input works correctly
- [ ] Keyboard input works
- [ ] Dark mode works
- [ ] Accessibility features work
- [ ] Background task handling works
- [ ] Memory usage is reasonable
- [ ] Performance is acceptable (60fps rendering)

### 9. TestFlight Feedback

#### Collecting Feedback

TestFlight provides built-in feedback:
- Testers can submit feedback via TestFlight app
- Screenshots and screen recordings can be attached
- Feedback is sent to App Store Connect

#### Feedback Management

1. Monitor TestFlight → Feedback in App Store Connect
2. Respond to testers
3. Track issues and bugs
4. Prioritize fixes for next build

### 10. Beta Testing Workflow

#### Release Cycle

1. **Development**: Implement features and fixes
2. **Internal Testing**: Test with team (1-2 days)
3. **External Beta**: Release to beta testers (1-2 weeks)
4. **Feedback Collection**: Gather and analyze feedback
5. **Iteration**: Fix issues and release new build
6. **App Store Submission**: When ready for production

#### Version Management

- Use semantic versioning: MAJOR.MINOR.PATCH
- Increment build number for each TestFlight upload
- Use version notes to communicate changes

### 11. TestFlight Automation (Optional)

#### Fastlane Integration

Create `Fastfile` for automation:

```ruby
lane :beta do
  increment_build_number
  build_app(scheme: "ZippyRemote")
  upload_to_testflight(
    skip_waiting_for_build_processing: false,
    distribute_external: true,
    notify_external_testers: true
  )
end
```

Run with:
```bash
fastlane beta
```

### 12. TestFlight Best Practices

#### Communication

- Send welcome email to testers
- Provide testing instructions
- Set expectations for feedback
- Regular updates on progress

#### Testing Focus

- Core functionality
- Performance and stability
- User experience
- Edge cases and error handling
- Different device types and iOS versions

#### Build Frequency

- Internal: Daily or as needed
- External: Weekly or bi-weekly
- Don't overwhelm testers with too many builds

### 13. TestFlight Limitations

- **Build Expiration**: Builds expire after 90 days
- **Tester Limits**: 
  - Internal: 100 testers
  - External: 10,000 testers
- **Build Processing**: Can take 15-30 minutes
- **Beta Review**: External testing requires App Review (24-48 hours)

### 14. Troubleshooting

#### Common Issues

**Build Processing Failed**
- Check build logs in App Store Connect
- Verify code signing
- Check for missing required files

**Testers Can't Install**
- Verify tester is added to group
- Check build is assigned to group
- Ensure build processing is complete

**App Crashes on Launch**
- Check crash logs in App Store Connect
- Verify all dependencies are included
- Check for missing resources

### 15. Transition to App Store

When ready for App Store release:

1. Finalize App Store metadata
2. Prepare screenshots and preview video
3. Submit for App Review
4. Monitor review status
5. Release when approved

## TestFlight Configuration Files

### .testflight.yml (if using automation)

```yaml
app_id: "1234567890"
api_key: "${TESTFLIGHT_API_KEY}"
api_issuer: "${TESTFLIGHT_ISSUER_ID}"
groups:
  - name: "Internal Testers"
    internal: true
  - name: "Beta Testers"
    internal: false
```

## Notes

- TestFlight builds are separate from App Store builds
- Testers need TestFlight app installed
- Feedback is valuable for improving the app
- Use TestFlight to validate before App Store submission
