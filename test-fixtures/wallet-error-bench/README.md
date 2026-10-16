# Wallet error bench — what a dApp sees when a broadcast fails

Built for the B5-T1-P2 owner sitting (2026-10-04, contract §4d-1 in
`development-docs/0.4.0-beta.7/track-1-money-path/phase-P2-a-failure-says-it-failed/`). Reusable for
any phase that needs a real tab to watch a wallet failure: the error the page catches, the gold pill,
the time at the bridge.

| File | What it is |
|---|---|
| `index.html` | Test page. **1a** `window.CWI.createAction`, **1b** the same over HTTP port 3321, **3** a real `noSend` + `signAction(sendWith)` flow. Prints exactly what the page received, plus `shimIsNewBuild` (is the current `window.CWI` shim running?) |
| `fake_broadcaster_switch.py` | Applies / reverts a **scratch-only** patch to `rust-wallet/src/services/mod.rs`: with `HODOS_DEV=1` and `HODOS_P2_FAKE_BROADCAST=temporary\|permanent\|hang`, the broadcast chain is replaced by a fake. Unset ⇒ normal. ⛔ **Never commit the applied patch** — revert before any commit (`git status` must not show `services/mod.rs`) |

## Run it

1. `python fake_broadcaster_switch.py <worktree>/rust-wallet apply`, then `cargo build --release`.
2. Start the dev wallet with `HODOS_DEV=1` and the mode you want; start ad-block, the frontend
   (`npm run dev`) and the dev browser as usual. Restart only the wallet to change mode.
3. `window.CWI` is injected only on a **main-frame `https://` page that is not localhost**
   (`simple_render_process_handler.cpp`). Serve this folder and tunnel it:
   `python -m http.server 8789 --bind 127.0.0.1` and `npx --yes localtunnel --port 8789`.
   localtunnel asks once for a password: the machine's public IP (`https://loca.lt/mytunnelpassword`).
4. Open the printed URL **in the dev browser**, approve the connect prompt, click.
5. Afterwards: `python fake_broadcaster_switch.py <worktree>/rust-wallet revert` and rebuild.

## Residue to expect

- **temporary / hang:** signed transactions are left `sending`. A normal wallet will really broadcast
  them; each one costs the 1,000-sat service fee plus the network fee. `TaskCheckForProofs` otherwise
  rolls them back after ~5–6 min of a three-indexer "not found".
- **permanent:** the row is `failed` and its coins are restored. Nothing is sent.
- ⚠️ The wallet DB is the real **dev** DB. `APPDATA` does not redirect it, so check the wallet's
  `Initializing database at:` log line.
