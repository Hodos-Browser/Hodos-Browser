# Service Fee Disclosure

This Software, as distributed by Marston Enterprises, includes a built-in
service fee of **1000 satoshis** sent to a Marston Enterprises BSV address on
every outgoing transaction (with the exception of backup-token transactions).
This fee funds ongoing development of the Hodos Browser.

## Where it is defined

The fee address and amount are constants in
[`rust-wallet/src/handlers.rs`](./rust-wallet/src/handlers.rs) —
`HODOS_FEE_ADDRESS` and `HODOS_SERVICE_FEE_SATS`. They are applied in the
transaction-building paths, including the certificate publish/unpublish
handlers (`rust-wallet/src/handlers/certificate_handlers.rs`) and dust
consolidation (`rust-wallet/src/monitor/task_consolidate_dust.rs`).

## If you fork this Software

You are responsible for either:

1. Removing or modifying the service fee logic to direct fees to your own
   address, or
2. Clearly disclosing to your users that transactions made through your fork
   will continue to pay Marston Enterprises.

Failing to do either of the above may mislead your users about where their
funds are going.

See also: [`LICENSE`](./LICENSE) (the code grant),
[`TRADEMARK.md`](./TRADEMARK.md) (name and branding),
[`COPYRIGHT`](./COPYRIGHT) (third-party components).
