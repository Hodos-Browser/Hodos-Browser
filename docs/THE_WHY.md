# The Why: Architectural and Language Choices

> **Purpose**: This document articulates the rationale behind our architectural and language decisions, and the sources behind them.

**Last Updated**: 2026-09-30

> **How this was checked (2026-09-30).** Statements about Hodos code were re-read against the code at
> `4d699ab` (code reading; nothing was run unless marked *measured*). External statistics and quotes
> were checked against the linked primary source on 2026-09-30 and are worded to match its scope.
> Claims we could not source were removed.

---

## Executive Summary

We are building a **production-grade BSV wallet** that handles real money. This requires security-first architecture that goes beyond what JavaScript-based solutions can provide. Our choices are:

1. **Native Wallet Backend** (Rust) - Process isolation, memory safety, per-site permission checks enforced in Rust
2. **Rust Language** - Memory safety without garbage collection, performance comparable to C/C++
3. **CEF with Process Isolation** - Multi-process architecture with security boundaries

**The Core Trade-off**: We chose security and correctness over development speed. Yes, there is tested TypeScript code for BSV wallets (`@bsv/sdk` and `@bsv/wallet-toolbox`). But we're building for production where real money is at stake, not just convenience.

---

## 1. Why Native Wallet Backend? (Security + UX)

### The Problem with JavaScript-Based Wallets

**The Reality**: JavaScript wallets operate in the browser's render process, which is inherently vulnerable to attack. Even with sandboxing, the attack surface is enormous.

#### 1.1 Process Isolation Vulnerabilities

**JavaScript's Limitation**: JavaScript runs in the browser's render process, which is accessible to web content. This creates a fundamental security boundary problem.

**Our Solution**: Native wallet operations run in isolated processes completely separate from web content.

**Why This Matters**:
- **Chromium's Multi-Process Architecture**: Even Chrome's sandboxing can't fully protect JavaScript from malicious websites. Extensions, injected scripts, and XSS attacks can still access the render process.
- **CEF's Security Boundaries**: By using CEF's native process model, we leverage Chromium's natural security boundaries between processes.
- **Attack Surface Reduction**: Even if a website compromises the render process, it cannot read the wallet's memory or keys, which live in a separate native process. It can still make the wallet calls that the site has been allowed to make (§1.3).

**Research Validation**:
- Chromium's design runs the rendering engine in a sandboxed, low-privilege process separate from the browser kernel (see: Barth, Jackson, Reis et al., [The Security Architecture of the Chromium Browser](https://seclab.stanford.edu/websec/chromium/chromium-security-architecture.pdf), 2008), and Site Isolation puts pages from different sites in separate processes (see: [Chromium Site Isolation](https://www.chromium.org/Home/chromium-security/site-isolation/))
- A separate native process has its own address space, so code running in a renderer cannot read the wallet's memory without a further exploit that escapes its own process (inference from the process model above)

#### 1.2 Memory Security Issues

**JavaScript's Limitation**: Private keys stored in JavaScript variables are accessible through:
- Browser console inspection (`console.log`, developer tools)
- Memory dumps and debugging tools
- Developer extensions and injected scripts
- Malicious scripts running in the same context

**Example Attack Vector**:
```javascript
// Even with encryption, this is vulnerable:
const encryptedKey = encrypt(masterKey);
// The masterKey variable is still in memory!
// Browser extensions can access it via:
//   - Memory inspection
//   - Console access
//   - Extension injection
```

**Our Solution**:

> Signing keys never leave the Rust process. No EC private key is ever returned to JavaScript, and no signing happens there. The BIP39 recovery phrase is the deliberate exception, in three places: it is shown once at wallet creation so the user can record it; it can be revealed again in the wallet UI (`POST /wallet/reveal-mnemonic`), which re-verifies the PIN when the wallet has one; and it is included, encrypted, in the file export (`POST /wallet/export`). Web pages cannot call the reveal route: the Rust wallet keeps it on a first-party-only list (`rust-wallet/src/main.rs`). That list treats any `localhost` / `127.0.0.1` origin, and any local caller that sends no requesting-domain header, as Hodos's own UI; this is a known issue scheduled for beta.7 Track 5 (see "Known gaps" below).

Concretely:
- **No private key material in the render process**: the key derivation and signing primitives live in `rust-wallet/src/crypto/`; other Rust modules also call `sign_ecdsa` directly (19 call sites outside `crypto/`, *measured*), all inside the wallet process. No secp256k1/ECDSA private-key code exists anywhere in `cef-native/`. The recovery phrase reaches the wallet UI through the same generic IPC bridge as other wallet calls; what keeps web pages from it is the Rust first-party-only check above, not where the bridge is injected.
- **Memory handling**: private keys are confined to the wallet process; the recovery phrase is also shown in the wallet UI, as above. Note that Rust's `Drop` deallocates but does not zero memory, and the wallet does not yet zeroize secrets — there is no `zeroize`/`secrecy` dependency, and `WalletDatabase::clear_cached_mnemonic()` (`rust-wallet/src/database/connection.rs`) merely drops the `Option<String>`, and is called only on wallet delete. Adding zeroization is ticketed for beta.8 (`TICKET_wallet_secrets_in_memory_are_never_wiped.md`).
- **No Developer Tools Access**: Native processes aren't accessible via browser console

**Research Validation**:
- Memory-unsafe code is the largest single source of disclosed vulnerabilities: "Memory safety vulnerabilities are the most prevalent type of disclosed software vulnerability" (CISA, NSA, FBI and partner agencies, [The Case for Memory Safe Roadmaps](https://www.cisa.gov/sites/default/files/2023-12/The-Case-for-Memory-Safe-Roadmaps-508c.pdf), December 2023)
- JavaScript gives a program no reliable way to wipe a secret: strings are immutable, and the garbage collector decides when memory is freed (general knowledge of the language; not tested here)
- Native processes can use platform secure-memory APIs (for example, locking pages out of swap). The Hodos wallet does not use them yet (see the zeroization ticket above)

**Concrete Example (Hodos Browser)**:
- Our Rust wallet uses **DPAPI** (Windows Data Protection API) / **Keychain** (macOS) to encrypt the mnemonic at rest at the OS level. On startup the wallet auto-unlocks (`rust-wallet/src/main.rs` → `WalletDatabase::try_dpapi_unlock`) and holds the plaintext mnemonic in Rust process memory for the life of the unlocked session (`WalletDatabase::cached_mnemonic`, `rust-wallet/src/database/connection.rs`), where it is used for key derivation. The plaintext is never reachable from web-page JavaScript; it does leave the native process when the wallet UI asks for it, at creation and on reveal (above). It is held for the session, not decrypted per use. JavaScript has no equivalent — `window.crypto.subtle` can encrypt but can't protect the key in memory from DevTools, extensions, or XSS.

#### 1.3 Cross-Site Scripting (XSS) Attack Surface

**JavaScript's Limitation**: XSS attacks can access wallet functions if the wallet code runs in the same context as web content.

**Express.js Cannot Fully Prevent This**:
- Express.js is a **backend framework** - it can't protect JavaScript code running in the browser
- Even with CSP headers, input validation, and output encoding, **JavaScript wallets are still vulnerable** because:
  - The wallet code itself runs in the browser's JavaScript context
  - Malicious websites can inject scripts that access wallet objects
  - Browser extensions can intercept and modify wallet operations
  - Same-origin policy doesn't protect against extension injection

**Our Solution**: Wallet operations don't run in JavaScript at all. They run in a native process that:
- **XSS cannot read** (different process, different security boundary). XSS cannot reach the keys or sign on its own; script running on a site the user has approved can make the calls that site is allowed, including payments within its spending limits, without a prompt.
- **Keeps key material out of every JavaScript context**, including injected and extension scripts
- **Exposes an API whose limits are enforced in Rust**. The injected bridge forwards any wallet path a page names; the Rust middleware decides what an external origin may reach (per-site permissions, plus the first-party-only list). Limiting approved sites to the dApp surface is scheduled for beta.7 Track 5 (phase P2). The injected JavaScript surfaces are inventoried in `cef-native/include/core/CLAUDE.md` and `cef-native/src/handlers/CLAUDE.md`.

**Research Validation**:
- Cross-site scripting is one of the most frequently reported web vulnerabilities. In the [OWASP Top 10:2025](https://top10.owasp.org/2025/) it sits under A05 Injection, which "includes Cross-site Scripting (high frequency/low impact) with more than 30k CVEs" ([A05:2025 Injection](https://top10.owasp.org/2025/A05_2025-Injection/))

**Concrete Example (Hodos Browser)**:
- Our **domain permission system** enforces per-domain spending limits in USD. The user sets limits for new sites and adjusts them per site. If XSS on an approved site triggered a `createAction` call, the request traverses three enforcement layers — none of which exist in a JavaScript-only wallet:
  1. **The C++ interceptor stamps the truth onto the request.** It attaches the true requesting origin and the pre-computed cost (`X-Requesting-Domain`, `X-Payment-Satoshis`, `X-Payment-Cents`, `X-Bsv-Price-Available`) in `cef-native/src/core/HttpRequestInterceptor.cpp`, so the page cannot lie about who it is or what it is spending.
  2. **The Rust permission engine decides.** Allow / prompt / deny is evaluated against the per-domain caps in `rust-wallet/src/permission_service/request_gate.rs :: dispatch_payment`, backed by the decision engine in `rust-wallet/crates/hodos_permission_engine/`.
  3. **The signing keys are not in a JavaScript context at all.** A page can reach signing only through the wallet's API, within the grants and limits the user has set for that site. Within them, signing and payment proceed without a prompt; beyond them, the page can only ask.

  The three layers are not independent: all of them rely on C++ classifying the caller as external. A caller classified as internal (Hodos's own UI, and today any `localhost` origin or header-free local caller) skips the per-site checks.

  (Historical note: the cap comparison used to live in a C++ `PermissionEngine` / `SessionManager`. Both were deleted in Phase 2.6-H, and the cap and grant decisions now live only in Rust. C++ decides internal versus external, builds partial context, forwards it, and renders whatever modal Rust asks for.)

### User Experience Benefits

**Native Wallet Advantages**:
1. **Performance**: No JavaScript interpretation overhead, direct machine code execution
2. **Responsiveness**: Native processes can handle cryptographic operations without blocking the UI
3. **Integration**: Better integration with OS-level security features (keychains, secure storage)
4. **Reliability**: No garbage collection pauses. Rust code can still fail at runtime (panics); the first-pass audit counted about 1,078 `.unwrap()` calls in the wallet (not re-measured)

---

## 2. Why Rust? (Speed + Security)

### The Question: Why Not TypeScript?

**The Reality**: There is tested TypeScript code for BSV wallets (`@bsv/sdk` and `@bsv/wallet-toolbox`); Hodos uses neither. However, we're building for production where:
- **Real money is at stake**
- **Security audits are required**
- **Memory safety is non-negotiable**
- **Performance matters** (cryptographic operations)

### 2.1 Memory Safety Without Garbage Collection

**Rust's Unique Advantage**: Rust provides memory safety guarantees at compile time without runtime overhead.

**Why This Matters for Cryptocurrency Wallets**:
- **No garbage collector**: No garbage collection pauses during transaction signing
- **Predictable Performance**: Memory management is deterministic, not probabilistic
- **Memory Clearing Is Possible**: Rust's ownership model allows explicit memory clearing of private keys (for example with the `zeroize` crate). The Hodos wallet does not do this yet (§1.2)
- **No Null Pointer Dereferences**: Compile-time guarantees prevent entire classes of vulnerabilities

**Research Validation**:
- Ownership "enables Rust to make memory safety guarantees without needing a garbage collector" ([The Rust Programming Language, ch. 4](https://doc.rust-lang.org/book/ch04-00-understanding-ownership.html))
- Microsoft's security team chose to explore Rust because "roughly 70% of the security issues that the MSRC assigns a CVE to are memory safety issues" ([MSRC, "Why Rust for safe systems programming"](https://www.microsoft.com/en-us/msrc/blog/2019/07/why-rust-for-safe-systems-programming), July 2019)

**Comparison with TypeScript**:
| Aspect | TypeScript | Rust |
|--------|-----------|------|
| Memory Safety | Runtime (GC) | Compile-time |
| Null Safety | Optional (`strictNullChecks`) | Guaranteed (no null by default) |
| Performance | JavaScript engine dependent | Native machine code |
| Memory Clearing | Garbage collected (no control) | Explicit ownership (can clear) |
| Security Guarantees | Runtime checks | Compile-time checks |

### 2.2 Performance Comparable to C/C++

**Rust's Performance**:
- **Zero-Cost Abstractions**: High-level code compiles to low-level machine code
- **No Runtime Overhead**: No garbage collector, no virtual machine
- **Direct Machine Code**: Compiles to native binaries, not interpreted

**Why Performance Matters**:
- **Transaction Signing**: Cryptographic operations must be fast (ECDSA signing, key derivation)
- **UTXO Management**: Large UTXO sets require efficient memory usage
- **Real-Time Operations**: Wallet operations shouldn't block the UI

We have not benchmarked Hodos's signing or key derivation against a JavaScript implementation, so this document gives no speed ratio.

### 2.3 Concurrency Without Data Races

**Rust's Concurrency Model**: Rust's ownership system ensures thread safety at compile time.

**Why This Matters**:
- **Asynchronous Operations**: Network I/O (UTXO fetching, broadcasting) runs asynchronously. The wallet deliberately serialises the select → sign → broadcast sequence of `createAction` behind `create_action_lock` (`rust-wallet/src/handlers.rs`), so two payments cannot pick the same coins
- **Thread Safety**: The compiler rejects code that shares data between threads without synchronisation. The wallet still uses locks (mutexes and `create_action_lock`); the compiler checks that they are used, not whether they are held for the right span
- **No Data Races**: Compile-time guarantees prevent entire classes of concurrency bugs

**Research Validation**:
- "By leveraging ownership and type checking, many concurrency errors are compile-time errors in Rust rather than runtime errors" ([The Rust Programming Language, ch. 16](https://doc.rust-lang.org/book/ch16-00-concurrency.html))

### 2.4 Security Audit Readiness

**Why Rust is Better for Security Audits**:
1. **Compile-Time Guarantees**: Many vulnerabilities are caught at compile time, not runtime
2. **Explicit Memory Management**: Auditors can verify memory clearing for sensitive data
3. **Type Safety**: Strong type system prevents entire classes of bugs
4. **No Undefined Behavior in Safe Rust**: Safe Rust code cannot cause undefined behavior. `unsafe` blocks and C dependencies are outside that guarantee and are where an auditor looks first

**Research Validation**:
- Google reports "a 1000x reduction in memory safety vulnerability density compared to Android's C and C++ code" for Rust in Android, and memory safety vulnerabilities "falling below 20% of total vulnerabilities for the first time" ([Google, "Rust in Android: move fast and fix things"](https://blog.google/security/rust-in-android-move-fast-fix-things/), November 2025). This is one codebase's vulnerability data, not audit findings

---

## 2.5 Why Rust Instead of C++? (The C++ Question)

### The Question: "We Use C++ for CEF - Why Not C++ for the Wallet Too?"

**The Reality**: We already use C++ for the CEF browser shell. So why not use C++ for the wallet backend too? This would:
- Reduce language diversity (one less language to maintain)
- Leverage existing C++ expertise
- Simplify the codebase (all native code in one language)

**Our Answer**: Rust provides critical security guarantees that C++ cannot, which are essential for wallet software handling real money.

### 2.5.1 Memory Safety: C++ vs Rust

**C++'s Limitation**: C++ requires manual memory management, which introduces entire classes of vulnerabilities:
- **Buffer Overflows**: C++ doesn't prevent out-of-bounds array access
- **Use-After-Free**: Manual memory management allows dangling pointer access
- **Null Pointer Dereferences**: C++ allows null pointers everywhere
- **Double Free**: Manual `delete` calls can corrupt memory

**Rust's Guarantee**: In safe Rust, the ownership system prevents all of these at compile time:
- **Buffer Safety**: Bounds checking prevents out-of-bounds access
- **Lifetime Tracking**: Compiler ensures memory is valid before use
- **No Null**: Option types prevent null pointer dereferences
- **Automatic Cleanup**: RAII ensures memory is freed exactly once

**Real-World Impact**:
- **Microsoft**: "approximately 70% of security vulnerabilities that Microsoft fixes and assigns a CVE are due to memory safety issues" ([MSRC, "We need a safer systems programming language"](https://www.microsoft.com/msrc/blog/2019/07/we-need-a-safer-systems-programming-language), July 2019)
- **Chromium**: "Around 70% of our high severity security bugs are memory unsafety problems (that is, mistakes with C/C++ pointers)", based on 912 high or critical severity bugs since 2015 ([Chromium, Memory safety](https://www.chromium.org/Home/chromium-security/memory-safety/))
- **Cryptocurrency wallets are prime targets** - memory safety bugs can lead to complete loss of funds

### 2.5.2 Security Audit Cost: C++ vs Rust

**C++ Security Audits**:
- Auditors must manually check every memory operation
- No compile-time guarantees mean runtime testing is required
- Vulnerabilities can be subtle and hard to find
- **Cost**: High (more time, more expertise required)

**Rust Security Audits**:
- Compiler has already caught most memory safety bugs
- Auditors focus on logic errors, not memory management
- Type system prevents entire classes of bugs
- **Cost**: Lower (fewer vulnerabilities to find, easier to verify)

These cost comparisons are our expectation; we have no audit data comparing the two (see §2.4 for the vulnerability-density data we do have).

### 2.5.3 Real-World Example: Trust Wallet's Wallet Core

**Trust Wallet's Decision**: Trust Wallet is migrating its Wallet Core library from **C++ to Rust** gradually, "starting with the first and biggest blockchain, Ethereum."

**Their Reasons** (from their blog post):
1. **Memory Safety**: "Rust's ownership model and strong type system help prevent memory-related bugs, making it ideal for developing secure wallet software."
2. **Performance**: "Designed with performance comparable to C++, Rust allows developers to write efficient code capable of handling the demands of blockchain and wallet operations without sacrificing safety."

The post also lists concurrency, C-compatible FFI and the growing blockchain ecosystem among its reasons.

**Key Insight**: A cryptocurrency wallet team with an existing C++ codebase chose Rust for its wallet library.

**Source**: [Trust Wallet Blog: "A Huge Step Forward: WalletCore's Migration from C++ to Rust"](https://trustwallet.com/blog/developer/a-huge-step-forward-wallet-cores-migration-to-rust) (published 24 October 2023)

### 2.5.4 C++ for Browser Shell, Rust for Wallet: Why Both?

**Our Architecture**:
- **C++**: Browser shell (CEF integration, UI rendering, process management)
- **Rust**: Wallet backend (cryptographic operations, private key management)

**Why This Separation**:
1. **Different Security Requirements**:
   - Browser shell: UI rendering, process management, and two wallet-relevant jobs: classifying each caller as internal or external, and pricing payments
   - Wallet: Private keys, transaction signing (critical security risk)

2. **Different Risk Profiles**:
   - C++ shell: Most vulnerabilities affect UI and rendering. A bug in caller classification or payment pricing affects wallet security, because Rust's permission checks rely on both
   - Rust wallet: Vulnerabilities affect funds (catastrophic)

3. **Right Tool for Right Job**:
   - C++: Excellent for CEF integration (native API compatibility)
   - Rust: Excellent for security-critical cryptographic code

4. **Process Isolation**: The wallet runs in a separate process anyway, so language choice is independent

### 2.5.5 Development Complexity Trade-off

**C++ Advantages**:
- ✅ Single language (C++ for everything)
- ✅ Existing C++ expertise
- ✅ Mature ecosystem
- ✅ Faster initial development (no learning curve)

**C++ Disadvantages**:
- ❌ Manual memory management (security risk)
- ❌ More vulnerabilities to find and fix
- ❌ Higher security audit costs
- ❌ Runtime errors instead of compile-time errors

**Rust Advantages**:
- ✅ Compile-time memory safety (security guarantee)
- ✅ Fewer vulnerabilities (caught at compile time)
- ✅ Lower security audit costs
- ✅ Better long-term maintainability

**Rust Disadvantages**:
- ❌ Additional language to learn
- ❌ Steeper learning curve
- ❌ Slightly longer initial development

**Our Decision**: We chose security and correctness over development convenience. The extra complexity of learning Rust is an investment in:
- **Security**: Fewer vulnerabilities in production
- **Audit Readiness**: Easier and cheaper security audits
- **Long-term Maintainability**: Compile-time guarantees catch bugs early

### 2.5.6 Performance: C++ vs Rust

**The Reality**: Rust performance is comparable to C++ for our use case.

**Why Performance is Similar**:
- Both compile to native machine code
- Both have zero-cost abstractions
- Both avoid garbage collection overhead

We have not benchmarked the two for our workload.

**Conclusion**: Performance is not a reason to choose C++ over Rust. Both provide native-level performance.

---

## 3. Addressing Common Pushback

### "But TypeScript is Faster to Develop"

**Our Response**: Yes, TypeScript is faster to develop. But we're building for **production**, not prototyping. The trade-off is:
- **Development Speed** (TypeScript) vs **Production Security** (Rust)
- **Time to Market** vs **Security Audit Readiness**
- **Convenience** vs **Correctness**

**The Reality**: We're not building a prototype. We're building software that handles real money. The extra development time is an investment in:
- Security (fewer vulnerabilities)
- Performance (better user experience)
- Maintainability (compile-time guarantees catch bugs early)

### "But Express.js Can Prevent XSS"

**Our Response**: Express.js is a **backend framework**. It can't protect JavaScript code running in the browser. Even with:
- CSP headers
- Input validation
- Output encoding
- Security middleware

**JavaScript wallets are still vulnerable** because:
- The wallet code runs in the browser's JavaScript context
- XSS attacks can access wallet objects directly
- Browser extensions can intercept wallet operations
- Same-origin policy doesn't protect against extension injection

**Our Solution**: Don't run wallet code in JavaScript at all. Run it in a native process whose memory XSS can't read, and whose API checks each site's permissions in Rust.

### "But the TypeScript SDK is Tested"

**Our Response**: Yes, the TypeScript SDK is tested. But:
1. **Testing ≠ Production Security**: Tests verify functionality, not security guarantees
2. **Memory Safety**: TypeScript can't provide compile-time memory safety guarantees
3. **Security Audits**: Production wallets require security audits, which are easier with Rust's guarantees
4. **Real-World Attacks**: JavaScript vulnerabilities are real and well-documented

**The Reality**: We're building our own implementation because:
- We need the wallet in its own process. That comes from the architecture rather than the language, and a wallet running in its own process is where Rust's guarantees pay off
- We need memory safety guarantees (TypeScript can't provide this)
- We need production-grade security (Rust's compile-time guarantees help)

### "But Native Processes are More Complex"

**Our Response**: Yes, native processes are more complex. But:
- **Security Requires Complexity**: Simple solutions are often insecure
- **CEF Handles Complexity**: We're using CEF, which handles process management for us
- **Investment in Security**: The complexity is an investment in security and correctness

### "But We Already Use C++ - Why Add Rust?"

**Our Response**: We use C++ for the browser shell (CEF integration) and Rust for the wallet backend. This is intentional:

- **Different Security Requirements**: The browser shell handles UI and rendering, plus caller classification and payment pricing (§2.5.4). The wallet handles private keys (critical risk).
- **Right Tool for Right Job**: C++ is excellent for CEF integration. Rust is excellent for security-critical code.
- **Process Isolation**: The wallet runs in a separate process anyway, so language choice is independent.
- **Real-World Example**: Trust Wallet is moving its Wallet Core library from C++ to Rust (§2.5.3).

**The Reality**: The extra complexity of learning Rust is an investment in security. Microsoft and Chromium each report that about 70% of their serious vulnerabilities are memory safety bugs (§2.5.1). For wallet software handling real money, Rust's compile-time guarantees are worth the learning curve.

**See Section 2.5 for detailed analysis of C++ vs Rust for wallet development.**

---

## 4. Architecture Validation

### 4.1 Multi-Process CEF Architecture

**Why CEF with Process Isolation**:
- **Chromium's Security Model**: Leverages Chromium's proven multi-process security architecture
- **Process Boundaries**: Natural security boundaries between processes
- **Isolation**: Even if one process is compromised, others are protected

**Research Validation**:
- See §1.1: the Chromium security architecture paper and Chromium's Site Isolation page

### 4.2 Controlled API Exposure

**Our Approach**: Page JavaScript receives a small set of injected functions, and every wallet call it makes is checked in Rust against that site's permissions. The injected surfaces are inventoried in `cef-native/include/core/CLAUDE.md` and `cef-native/src/handlers/CLAUDE.md` — root docs deliberately do not enumerate them.

**Why This Matters**:
- **Principle of Least Privilege**: Only expose what's necessary. Today exposure is a deny-list (the first-party-only routes); moving to an allowlist of the BRC-100 (Unified, Vendor-Neutral, Unchanging, and Open BSV Blockchain Standard Wallet-to-Application Interface) surface is scheduled for beta.7 Track 5, phase P2
- **No Sensitive Data**: See the statement in §1.2. Signing keys never leave the Rust process; the recovery phrase reaches only the wallet UI and the encrypted export.
- **Controlled Interface**: All wallet operations go through one API, and the Rust middleware is where its limits are enforced

---

## 5. Real-World Examples

### 5.1 JavaScript Wallet Vulnerabilities

**Attack classes**:
- **Browser Extension Attacks**: Extensions can inject scripts that access wallet objects
- **XSS Attacks**: Malicious websites can access wallet functions via XSS
- **Memory Dumps**: Developer tools can inspect wallet memory
- **Console Access**: Private keys can be accessed via browser console

**Our Defense**: Native process isolation keeps private keys out of reach of all four. It does not stop a script on an approved site from using that site's grants (§1.3).

### 5.2 Production Cryptocurrency Wallets

**Industry Examples**:
- **Ledger**: Native firmware (not JavaScript)
- **Trezor**: Native firmware (not JavaScript)
- **MetaMask**: JavaScript-based, but requires browser extension security model
- **Electrum**: Python desktop application (not JavaScript)

**Our Approach**: Native Rust wallet in a separate local process, reached from the browser through a permission-checked API. Hardware wallets such as Ledger and Trezor go further: their keys sit on a separate device, which Hodos does not do.

---

## 6. Conclusion

**Our Choices Rest On**:
1. **Security Research**: Process isolation and memory safety are the approaches the Chromium project, Microsoft and the CISA/NSA guidance cited above recommend
2. **Performance Requirements**: Cryptographic operations require native performance
3. **Security Audit Readiness**: Rust's guarantees make security audits easier

**The Trade-off**:
- **Development Speed** (TypeScript) vs **Production Security** (Rust)
- **Convenience** (JavaScript) vs **Correctness** (Native + Rust)

**We Chose**: Security and correctness. Because we're building software that handles real money.

---

## Known gaps (2026-09-30)

These are recorded here because the argument above depends on them. Each has a ticket.

| Gap | Ticket | Release |
|---|---|---|
| Any `localhost` / `127.0.0.1` page, and any local caller that sends no requesting-domain header, is treated as Hodos's own UI and skips every per-site check, including the first-party-only list | `development-docs/0.4.0-beta.7/tickets/TICKET_headerless_loopback_requests_are_trusted_as_wallet_ui.md` | beta.7, Track 5 phase P2 |
| An approved site can call any wallet route not on the first-party-only list (deny-list, not allowlist) | `development-docs/0.4.0-beta.7/tickets/TICKET_dapp_reachable_surface_is_a_denylist_not_an_allowlist.md` | beta.7, Track 5 phase P2 |
| Secrets in memory are never wiped (no zeroization) | `development-docs/0.4.0-beta.8/tickets/TICKET_wallet_secrets_in_memory_are_never_wiped.md` | beta.8 intake |
| The Centbee import creates a wallet with no PIN; its recovery phrase is stored in plaintext in `wallet.db`, and reveal returns it with no check | `development-docs/0.4.0-beta.8/tickets/TICKET_centbee_import_creates_a_wallet_with_no_pin.md` | beta.8 intake |

---

## References

1. **The Security Architecture of the Chromium Browser** (Barth, Jackson, Reis et al., 2008): https://seclab.stanford.edu/websec/chromium/chromium-security-architecture.pdf
2. **Chromium Site Isolation**: https://www.chromium.org/Home/chromium-security/site-isolation/
3. **Chromium, Memory safety**: https://www.chromium.org/Home/chromium-security/memory-safety/
4. **MSRC, We need a safer systems programming language** (July 2019): https://www.microsoft.com/msrc/blog/2019/07/we-need-a-safer-systems-programming-language
5. **MSRC, Why Rust for safe systems programming** (July 2019): https://www.microsoft.com/en-us/msrc/blog/2019/07/why-rust-for-safe-systems-programming
6. **CISA, NSA et al., The Case for Memory Safe Roadmaps** (December 2023): https://www.cisa.gov/sites/default/files/2023-12/The-Case-for-Memory-Safe-Roadmaps-508c.pdf
7. **Google, Rust in Android: move fast and fix things** (November 2025): https://blog.google/security/rust-in-android-move-fast-fix-things/
8. **Trust Wallet, A Huge Step Forward: WalletCore's Migration from C++ to Rust** (October 2023): https://trustwallet.com/blog/developer/a-huge-step-forward-wallet-cores-migration-to-rust
9. **OWASP Top 10:2025, A05 Injection**: https://top10.owasp.org/2025/A05_2025-Injection/
10. **The Rust Programming Language** (ch. 4 Ownership, ch. 16 Concurrency): https://doc.rust-lang.org/book/
11. **Express.js Security**: https://expressjs.com/en/advanced/best-practice-security.html
12. **CEF Documentation**: https://bitbucket.org/chromiumembedded/cef/wiki/Home.md

---

**Last Updated**: 2026-09-30
**Status**: Living document - updated as we learn and validate our choices
