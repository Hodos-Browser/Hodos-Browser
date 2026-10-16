# Hodos Browser reference docs

These documents describe how Hodos Browser works today: its architecture, its security model and the reasons behind its design. They are updated each release. Where a doc and the code disagree, the code is right and the doc needs fixing.

| Document | What it covers |
|----------|----------------|
| [`PROJECT_OVERVIEW.md`](PROJECT_OVERVIEW.md) | The architecture across all three layers (React frontend, C++ CEF shell, Rust wallet): how requests flow, where keys and permissions live, and pointers to the per-layer docs |
| [`THE_WHY.md`](THE_WHY.md) | Why Hodos uses Rust for the wallet, CEF for the browser and a separate native wallet process, with the sources behind each choice |
| [`SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md`](SECURITY_AND_PROCESS_ISOLATION_ANALYSIS.md) | The process map, the trust level of each process class, and what each boundary does and does not stop |
| [`architecture/`](architecture/README.md) | Flows that cross the React / C++ / Rust boundary |
| [`architecture/WALLET_API_MAP.md`](architecture/WALLET_API_MAP.md) | Which permission gate each wallet endpoint passes through |
| [`architecture/IPC_BRIDGE.md`](architecture/IPC_BRIDGE.md) | How page and UI calls travel from the renderer through the C++ shell to the Rust wallet |
| [`architecture/AUTO_APPROVE_ENGINE.md`](architecture/AUTO_APPROVE_ENGINE.md) | The Rust permission decision engine: when a call is silent, when it prompts, and how spending caps apply |

Build guides are in [`../build-instructions/`](../build-instructions/). [`../development-docs/`](../development-docs/) holds internal release planning and tickets; known issues and the release each is planned for are listed in the [root README](../README.md#known-issues).
