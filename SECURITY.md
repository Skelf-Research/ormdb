# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take the security of ORMDB seriously. If you believe you have found a security vulnerability, please report it to us as described below.

### How to Report

**Please do not report security vulnerabilities through public GitHub issues.**

Instead, please send an email to **security@skelfresearch.com** with:

1. **Description** of the vulnerability
2. **Steps to reproduce** the issue
3. **Potential impact** of the vulnerability
4. **Suggested fix** (if you have one)

### What to Expect

- **Acknowledgment**: We will acknowledge receipt of your report within 48 hours
- **Initial Assessment**: We will provide an initial assessment within 7 days
- **Resolution Timeline**: We aim to resolve critical vulnerabilities within 30 days
- **Credit**: We will credit reporters in our security advisories (unless you prefer to remain anonymous)

### Safe Harbor

We consider security research conducted in accordance with this policy to be:

- Authorized concerning any applicable anti-hacking laws
- Authorized concerning any relevant anti-circumvention laws
- Exempt from restrictions in our Terms of Service that would interfere with conducting security research

We will not pursue civil action or initiate a complaint against researchers who:

- Engage in testing within the scope of this policy
- Avoid privacy violations, data destruction, and service disruption
- Do not exploit vulnerabilities beyond what is necessary to demonstrate them
- Report vulnerabilities promptly

## Security Best Practices

When deploying ORMDB in production:

1. **Network Security**
   - Run ORMDB behind a firewall
   - Use TLS for all client connections
   - Restrict network access to trusted clients

2. **Authentication**
   - Enable authentication for all connections
   - Use strong, unique credentials
   - Rotate credentials regularly

3. **Authorization**
   - Apply the principle of least privilege
   - Use row-level security where appropriate
   - Audit permission grants regularly

4. **Data Protection**
   - Enable encryption at rest
   - Back up data regularly
   - Test restore procedures

5. **Monitoring**
   - Enable audit logging
   - Monitor for unusual access patterns
   - Set up alerts for security events

## Security Updates

Security updates are released as patch versions. We recommend:

- Subscribing to our [security advisories](https://github.com/Skelf-Research/ormdb/security/advisories)
- Enabling automatic updates for patch versions
- Testing updates in a staging environment before production

## Contact

- Security issues: security@skelfresearch.com
- General inquiries: support@skelfresearch.com
