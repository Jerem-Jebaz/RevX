# Threat Model

NoUSB assumes boot configuration is high-value system state. The product must not become a persistence mechanism, policy bypass, or unsafe downloader.

## In Scope

- Prevent accidental boot misconfiguration.
- Prevent unverified installer execution.
- Redact secrets from logs.
- Avoid hidden telemetry.
- Preserve user agency with explicit confirmations.
- Keep default boot target unchanged where possible.

## Out Of Scope

- Bypassing Secure Boot, SIP, TPM, BitLocker, FileVault, activation, licensing, or vendor policy.
- Storing raw recovery keys unless a future encrypted vault is explicitly implemented.
- Firmware modification.
- Stealth persistence.

