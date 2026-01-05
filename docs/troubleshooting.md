# Troubleshooting

Common issues and solutions for ZRC.

## Connection Issues

### "Unable to connect to rendezvous server"

**Cause**: Network connectivity or server unavailable.

**Solutions**:
1. Check your internet connection
2. Verify the rendezvous URL is correct
3. Check if a firewall is blocking HTTPS (port 443/8443)
4. If self-hosting, ensure the server is running

### "Pairing failed"

**Cause**: Pairing code expired or mistyped.

**Solutions**:
1. Pairing codes expire after 5 minutes - generate a new one
2. Ensure you're entering the code exactly as shown
3. Check that both devices have internet access

### "Connection timed out"

**Cause**: NAT traversal failed.

**Solutions**:
1. Ensure relay servers are configured and reachable
2. Check UDP port 4433 is not blocked
3. Try a different network (some corporate firewalls block QUIC)

## Performance Issues

### Laggy or choppy display

**Solutions**:
1. Reduce display quality in settings
2. Check network bandwidth (recommend 5+ Mbps)
3. Use wired connection instead of WiFi
4. Close bandwidth-heavy applications

### High CPU usage on host

**Solutions**:
1. Reduce capture frame rate
2. Disable hardware acceleration if causing issues
3. Check for other screen recording software conflicts

## Platform-Specific Issues

### Windows: "Access denied" when capturing screen

**Cause**: Missing permissions or security software blocking.

**Solutions**:
1. Run agent as Administrator for initial setup
2. Add exception in antivirus/security software
3. Check Windows Privacy settings for screen recording

### macOS: Black screen when connecting

**Cause**: Screen recording permission not granted.

**Solutions**:
1. Go to System Preferences > Security & Privacy > Privacy
2. Select "Screen Recording" and enable for ZRC Agent
3. Restart the agent after granting permission

### Linux: No input control

**Cause**: Missing uinput permissions.

**Solutions**:
1. Add user to `input` group: `sudo usermod -a -G input $USER`
2. Log out and back in
3. For Wayland, additional portal permissions may be needed

## Logs

### Log Locations

- **Windows**: `%APPDATA%\ZippyRemote\logs\`
- **macOS**: `~/Library/Logs/ZippyRemote/`
- **Linux**: `~/.local/share/zippyremote/logs/`

### Enable Debug Logging

```bash
ZRC_LOG_LEVEL=debug zrc-agent --foreground
```

## Getting Help

1. Check the [GitHub Issues](https://github.com/GoZippy/ZippyViewer/issues) for known problems
2. Search closed issues for solutions
3. Open a new issue with:
   - OS and version
   - ZRC version
   - Steps to reproduce
   - Relevant log excerpts
