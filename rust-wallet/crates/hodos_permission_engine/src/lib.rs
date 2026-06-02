//! hodos_permission_engine — pure decision logic for the wallet permission engine.
//!
//! Scaffolding stub for sub-phase 2.6-A.1 (workspace conversion). Real types and
//! decision logic land in sub-phases 2.6-A.2 (context + decision types),
//! 2.6-A.3 (decide() body and Matrix C branches), and 2.6-A.4 (ported tests).
//!
//! Architecture:
//!   - PURE LOGIC. No actix, no sqlite, no http, no AppState dependency.
//!   - Mirrors the C++ `PermissionEngine` at `cef-native/src/core/PermissionEngine.cpp`.
//!   - Becomes the canonical implementation during Phase 2.6; C++ engine deleted in 2.6-H.
//!
//! See: `development-docs/Sigma-BRC121-Sprint/phase-2.6-engine-to-rust/`
