# Flatpak Limitations for ZRC Agent

## Overview

While ZRC Agent can be packaged as a Flatpak, there are significant limitations due to Flatpak's sandboxing model.

## Limitations

### 1. Screen Capture
- **X11**: Requires `--socket=x11` permission, but may not work reliably in all environments
- **Wayland**: Requires `--socket=wayland` and portal permissions, which should work but requires user interaction

### 2. Input Injection
- **X11**: Requires `--socket=x11` permission, but input injection may be blocked by the sandbox
- **Wayland**: Input injection is severely limited in Flatpak due to security restrictions
- **uinput**: Not available in Flatpak sandbox

### 3. System Integration
- **systemd**: Limited access to systemd services from within Flatpak
- **Secret Service**: May work but requires proper portal permissions
- **Clipboard**: Requires portal permissions

### 4. Network Access
- Requires `--share=network` permission

## Recommended Approach

For full functionality, ZRC Agent should be installed as a traditional package (.deb, .rpm) or AppImage rather than Flatpak.

If Flatpak support is required, consider:
1. Using portal APIs for all privileged operations
2. Accepting reduced functionality (view-only mode)
3. Requesting additional permissions in the manifest

## Flatpak Manifest Example

```yaml
app-id: com.zrc.agent
runtime: org.freedesktop.Platform
runtime-version: '22.08'
sdk: org.freedesktop.Sdk
command: zrc-agent

finish-args:
  - --socket=x11
  - --socket=wayland
  - --share=network
  - --talk-name=org.freedesktop.secrets
  - --talk-name=org.freedesktop.portal.Desktop
  - --talk-name=org.freedesktop.portal.ScreenCast
```

## Conclusion

Flatpak support is possible but with significant limitations. For production use, traditional packages or AppImage are recommended.
