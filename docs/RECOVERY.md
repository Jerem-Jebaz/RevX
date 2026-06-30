# Recovery

Recovery packs are written to `recovery-packs/pack-<timestamp>`.

The current pack includes:

- Host OS and architecture.
- Firmware and Secure Boot state as detected by the safe adapter.
- Encryption status placeholder.
- Manual recovery instructions.

This is not a full user-data backup. Replace-current-OS flows must require acknowledgement that a verified external user-data backup exists before real adapters are enabled.

## Diagnostics

Use:

```sh
cargo run -p nousb -- diagnostics export
```

Diagnostic bundles are written to `.nousb/diagnostics/bundle-<timestamp>`. They are not recovery packs, but they help inspect local operation state. Secret-like log lines are redacted before export.
