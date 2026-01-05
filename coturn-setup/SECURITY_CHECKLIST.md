# coturn Security Checklist

Use this checklist to ensure secure deployment of coturn.

## Pre-Deployment Security

### Secrets and Authentication
- [ ] Strong `TURN_SECRET` generated (32+ bytes, random)
- [ ] `TURN_SECRET` stored securely (not in git)
- [ ] `.env` file added to `.gitignore`
- [ ] Credential generation tested
- [ ] Authentication mechanism verified

### TLS Configuration
- [ ] TLS certificates obtained (Let's Encrypt for production)
- [ ] Certificate paths configured correctly
- [ ] Certificate and key match verified
- [ ] Certificate expiration date checked
- [ ] Certificate renewal automated (Let's Encrypt)
- [ ] Certificate permissions set (644 for cert, 600 for key)

### Network Security
- [ ] `EXTERNAL_IP` set to correct public IP
- [ ] Firewall rules configured (UDP 3478, TCP 3478, TCP 5349)
- [ ] Unnecessary ports closed
- [ ] Network isolation considered
- [ ] IP access control configured (if needed)

### Configuration Security
- [ ] `no-cli` enabled (CLI disabled)
- [ ] `no-loopback-peers` enabled
- [ ] `no-multicast-peers` enabled
- [ ] Resource limits configured appropriately
- [ ] Rate limiting configured
- [ ] IP allowlist/blocklist configured (if needed)

### Container Security
- [ ] Running as non-root user (65534:65534)
- [ ] Unnecessary capabilities dropped
- [ ] Only required capabilities added (NET_BIND_SERVICE)
- [ ] Container image from trusted source
- [ ] Container image regularly updated

## Deployment Security

### File Permissions
- [ ] Configuration files readable by container
- [ ] Certificate files have correct permissions
- [ ] Log directory writable by container
- [ ] No world-writable files

### Service Configuration
- [ ] Health checks configured
- [ ] Logging enabled and configured
- [ ] Log rotation configured
- [ ] Monitoring set up

### Network Configuration
- [ ] Network mode appropriate (host mode for TURN)
- [ ] Ports correctly mapped
- [ ] External IP correctly configured
- [ ] DNS resolution working

## Post-Deployment Security

### Validation
- [ ] STUN functionality tested
- [ ] TURN allocation tested
- [ ] TLS/TURNS tested
- [ ] Authentication tested
- [ ] Resource limits tested

### Monitoring
- [ ] Log monitoring configured
- [ ] Error alerting set up
- [ ] Resource usage monitoring
- [ ] Authentication failure monitoring
- [ ] Unusual activity detection

### Maintenance
- [ ] Certificate expiration monitoring
- [ ] Secret rotation schedule established
- [ ] Update procedure documented
- [ ] Backup procedure established
- [ ] Incident response plan documented

## Ongoing Security

### Regular Tasks
- [ ] Review logs weekly for errors
- [ ] Check certificate expiration monthly
- [ ] Rotate secrets quarterly (or as needed)
- [ ] Update coturn image regularly
- [ ] Review resource usage monthly
- [ ] Check for security advisories

### Security Audits
- [ ] Configuration reviewed quarterly
- [ ] Access logs reviewed monthly
- [ ] Resource limits reviewed quarterly
- [ ] Network security reviewed quarterly
- [ ] Incident response tested annually

## Incident Response

### Preparation
- [ ] Incident response plan documented
- [ ] Contact information updated
- [ ] Escalation procedures defined
- [ ] Backup and restore tested

### Response
- [ ] Incident detection procedures
- [ ] Containment procedures
- [ ] Investigation procedures
- [ ] Recovery procedures
- [ ] Post-incident review

## Compliance

### Documentation
- [ ] Security procedures documented
- [ ] Configuration documented
- [ ] Change management process
- [ ] Access control documented

### Auditing
- [ ] Log retention policy
- [ ] Audit log access
- [ ] Regular security reviews
- [ ] Compliance checks

## Notes

- Review this checklist before each deployment
- Update checklist as security requirements change
- Document any deviations with justification
- Regular security audits recommended

## Resources

- [SECURITY.md](SECURITY.md) - Detailed security guide
- [TROUBLESHOOTING.md](TROUBLESHOOTING.md) - Troubleshooting guide
- [CONFIGURATION.md](CONFIGURATION.md) - Configuration reference
