# HodosBrowser — MVP Testing Guide

**Purpose**: A human-friendly exploration guide for testing HodosBrowser. Not a 500-row spreadsheet — a set of missions that put you in a real user's shoes.

**How to use this**:
1. Pick a tier (start with Tier 1)
2. Read the mission briefing
3. Do the thing — like a real person would
4. Note anything that feels off (not just crashes — weird flows, confusing labels, ugly transitions)
5. Mark your progress and move on

**Tracking**:
- `[ ]` Not started
- `[~]` Explored, notes below
- `[x]` Explored, feels solid

**Golden rule**: If something makes you pause, squint, or say "wait, what?" — write it down. That's more valuable than confirming a button changes color on hover.

---

## Tier 1: First Impressions

*You just downloaded this browser. You know nothing. Go.*

---

### Mission 1.1 — Fresh Start

**Goal**: Launch the browser for the first time. Does it feel like a real browser?

- [ ] Launch HodosBrowserShell.exe — does a window appear quickly?
- [ ] Does the new tab page load? Does it look intentional or broken?
- [ ] Type `youtube.com` in the address bar and hit Enter
- [ ] Does the page load fully? Do videos play?
- [ ] Open a few tabs (Ctrl+T). Close some (Ctrl+W). Does it feel snappy?
- [ ] Try back/forward buttons. Do they work as expected?
- [ ] Click the address bar — is your cursor there? Can you type immediately?

**Vibe check**: On a scale of 1-5, how "real browser" does this feel on first launch?

```
Rating: ___/5
Notes:


```

---

### Mission 1.2 — Can I Watch YouTube?

**Goal**: Browse YouTube like you normally would. Spend 5-10 minutes.

- [ ] Search for a video. Click it. Does it play?
- [ ] Are ads blocked? (You should NOT see pre-roll ads)
- [ ] Try fullscreen. Does it work? Can you exit fullscreen?
- [ ] Click a recommended video. Does navigation feel normal?
- [ ] Try adjusting video quality (gear icon)
- [ ] Open a video in a new tab (right-click → Open in new tab)
- [ ] Play audio — does it come through? Volume control?

**Side quest**: Open YouTube in one tab and a news site (bbc.com, nytimes.com) in another. Switch between them. Any lag or weirdness?

```
Notes:


```

---

### Mission 1.3 — Can I Log Into Things?

**Goal**: Test authentication on real sites. This is where browsers often break.

- [ ] **GitHub**: Sign in with your account. Browse a repo. Can you see code? Can you star a repo?
- [ ] **Google**: Sign into a Google account. Does the login flow complete? (Note: Google FedCM may not work — document what happens)
- [ ] **x.com (Twitter)**: Sign in. Can you scroll the timeline? Load media?
- [ ] **Reddit**: Sign in. Browse a subreddit. Upvote something.
- [ ] After signing in, close the browser and reopen. Are you still logged in?

**What to watch for**: Login redirects that loop, OAuth popups that don't appear, "browser not supported" messages, sessions that don't persist.

```
Rating (Auth Experience): ___/5
Notes:


```

---

### Mission 1.4 — Daily Browsing Gauntlet

**Goal**: Use this as your actual browser for 30 minutes. Hit the sites you'd normally visit.

- [ ] Check email (Gmail, Outlook, etc.)
- [ ] Browse a news site — do articles load? Images? Videos embedded in articles?
- [ ] Visit Amazon or another shopping site — search, browse, add to cart
- [ ] Open a Google Doc or other web app — does it load? Can you type?
- [ ] Download a file (PDF, image, zip) — does the download work?
- [ ] Try a site with a complex layout (Figma, Notion, Trello)

**Free roam**: Just browse for a while. Note every moment where you think "a real browser wouldn't do this."

```
Notes:


```

---

## Tier 2: The Browser Basics

*You've decided to give this browser a real shot. Now test the features you rely on daily.*

---

### Mission 2.1 — Tab Wrangling

**Goal**: Push the tab system. Browsers live and die by tabs.

- [ ] Open 10+ tabs. Does performance hold up?
- [ ] Ctrl+Tab through them. Does it cycle correctly?
- [ ] Ctrl+W to close tabs. Does the right tab get focus after closing?
- [ ] Middle-click a link to open in new tab
- [ ] Right-click a tab — what options appear? Do they work?
- [ ] Drag a tab to reorder it. Does it move smoothly?
- [ ] Drag a tab out of the window (tear-off). Does a new window appear?
- [ ] Drag a tab from one window back into another (merge). Does it work?
- [ ] Close all tabs in a window — does the window close?
- [ ] Open a tab, navigate somewhere, close it. Is there "reopen closed tab" (Ctrl+Shift+T)?

**Stress test**: Open 20 tabs. Close them all rapidly. Any crashes or freezes?

```
Notes:


```

---

### Mission 2.2 — Multi-Window Life

**Goal**: Test multiple browser windows side-by-side.

- [ ] Open a second window (Ctrl+N or from menu)
- [ ] Each window should have its own tab bar and address bar
- [ ] Browse different sites in each window — do they stay independent?
- [ ] Close one window — does the other stay alive?
- [ ] Resize windows. Does content reflow?
- [ ] Minimize and restore a window. Does it come back correctly?

```
Notes:


```

---

### Mission 2.3 — Downloads

**Goal**: Download files like a normal user.

- [ ] Download a PDF from any site
- [ ] Download an image (right-click → Save Image)
- [ ] Download a .zip from GitHub (any release page)
- [ ] Open the Downloads panel (Ctrl+J or menu → Downloads)
- [ ] Can you see your downloads with progress?
- [ ] "Open" a completed download — does it launch?
- [ ] "Show in Folder" — does it open the right directory?
- [ ] Pause and resume a large download
- [ ] Cancel a download mid-progress
- [ ] Clear completed downloads from the list

```
Notes:


```

---

### Mission 2.4 — Find in Page

**Goal**: Ctrl+F should just work.

- [ ] Open a long article (Wikipedia works well)
- [ ] Ctrl+F → type a word → does it highlight matches?
- [ ] Does it show "X of Y" match count?
- [ ] Enter/arrows to jump between matches
- [ ] Escape to close the find bar
- [ ] Does the bar close cleanly? No leftover highlights?

```
Notes:


```

---

### Mission 2.5 — The Three-Dot Menu

**Goal**: Explore the main menu. Does everything in it actually work?

- [ ] Click the three-dot menu icon (top-right)
- [ ] **New Tab** — opens a tab?
- [ ] **Find** — opens find bar?
- [ ] **Print** — opens print dialog?
- [ ] **Zoom** — plus/minus/reset all work?
- [ ] **Bookmark this page** — adds a bookmark?
- [ ] **Downloads** — opens download panel?
- [ ] **History** — shows browsing history?
- [ ] **DevTools** — opens developer tools?
- [ ] **Settings** — opens settings page?
- [ ] **Exit** — closes the browser?
- [ ] Click outside the menu — does it close?
- [ ] Press Escape — does it close?

```
Notes:


```

---

### Mission 2.6 — Keyboard Shortcuts

**Goal**: Power users live by shortcuts. Do ours work?

| Shortcut | Expected | Works? |
|----------|----------|--------|
| Ctrl+T | New tab | [ ] |
| Ctrl+W | Close tab | [ ] |
| Ctrl+Tab | Next tab | [ ] |
| Ctrl+Shift+Tab | Previous tab | [ ] |
| Ctrl+L | Focus address bar | [ ] |
| Ctrl+R / F5 | Reload | [ ] |
| Ctrl+Shift+R | Hard reload | [ ] |
| Ctrl+F | Find in page | [ ] |
| Ctrl+J | Downloads | [ ] |
| Ctrl+H | History | [ ] |
| Ctrl+D | Bookmark page | [ ] |
| Ctrl+N | New window | [ ] |
| Ctrl+Shift+I / F12 | DevTools | [ ] |
| Alt+Left | Back | [ ] |
| Alt+Right | Forward | [ ] |
| Ctrl+Plus | Zoom in | [ ] |
| Ctrl+Minus | Zoom out | [ ] |
| Ctrl+0 | Reset zoom | [ ] |

```
Notes:


```

---

### Mission 2.7 — Context Menus (Right-Click)

**Goal**: Right-click everywhere. Are the options useful?

- [ ] Right-click on a page — what do you see?
- [ ] Right-click on a link — "Open in New Tab" works?
- [ ] Right-click on an image — "Save Image" works?
- [ ] Right-click on selected text — "Copy" works?
- [ ] Right-click on the address bar — paste works?
- [ ] Right-click on a tab — what options appear?

**What's missing?** List any right-click options you expected but didn't see:

```
Notes:


```

---

## Tier 3: Privacy Shield

*This is what makes HodosBrowser different from Chrome. Does the privacy layer actually work?*

---

### Mission 3.1 — Ad Blocking in the Wild

**Goal**: Browse ad-heavy sites. Are ads gone?

- [ ] **YouTube** — no pre-roll, mid-roll, or banner ads?
- [ ] **News sites** (nytimes.com, bbc.com, theverge.com) — ads stripped from articles?
- [ ] **Reddit** — promoted posts removed?
- [ ] **Random blog sites** — sidebar/popup ads gone?
- [ ] Check the Privacy Shield icon — does it show a blocked count?
- [ ] Click the shield — does it show breakdown of what was blocked?

**Side quest**: Find a site where blocking breaks something (images missing, layout broken, login fails). Note the URL.

```
Broken sites:


```

---

### Mission 3.2 — Privacy Shield Panel

**Goal**: Open the shield panel and explore the controls.

- [ ] Click the shield icon in the toolbar
- [ ] Toggle ad/tracker blocking off → reload the page → do ads appear?
- [ ] Toggle it back on → reload → ads gone again?
- [ ] Toggle cookie blocking — does it change behavior on cookie-heavy sites?
- [ ] Toggle scriptlet injection — does toggling break/fix any sites?
- [ ] Toggle fingerprint protection
- [ ] Close the panel (click outside) — does it close cleanly?

**Watch for**: Do toggles actually take effect immediately or only after reload? Is that clear to the user?

```
Notes:


```

---

### Mission 3.3 — Fingerprint Protection

**Goal**: Verify that fingerprint protection actually changes your fingerprint.

- [ ] Visit a fingerprint test site (search "browser fingerprint test")
- [ ] Note your canvas hash, WebGL renderer, hardware concurrency, etc.
- [ ] Close the browser and reopen — do fingerprint values change? (They should — new session)
- [ ] With fingerprint protection ON, do sites still work? (YouTube, Google, GitHub)
- [ ] Turn OFF fingerprint protection in settings → revisit test site → values should be "real" now

```
Fingerprint test site used:
Notes:


```

---

### Mission 3.4 — Settings: Privacy Section

**Goal**: Verify privacy settings persist and actually do something.

- [ ] Open Settings → Privacy & Security
- [ ] Toggle each setting. Close settings. Reopen. Are they still set?
- [ ] "Clear Browsing Data" — does it work? (History, cookies, cache)
- [ ] "Clear data on exit" toggle — enable it, close browser, reopen. Is data cleared?
- [ ] DNT (Do Not Track) header — enable it, visit `httpbin.org/headers`. Is `DNT: 1` present?

```
Notes:


```

---

## Tier 4: Wallet — Getting Started

*Time to test the Bitcoin wallet. Start simple.*

---

### Mission 4.1 — Create a Wallet

**Goal**: First-time wallet setup. Is it clear and confidence-inspiring?

- [ ] Click the wallet icon in the toolbar
- [ ] Does the wallet panel open?
- [ ] Choose "Create New Wallet"
- [ ] Is the mnemonic (seed phrase) displayed clearly? Can you read all 12 words?
- [ ] Is there a warning about writing them down?
- [ ] Set a PIN — is the PIN entry intuitive?
- [ ] Confirm the PIN
- [ ] Does the wallet open to the dashboard after setup?
- [ ] Is your balance showing? (Should be 0 BSV for new wallet)
- [ ] Is a receive address displayed?

**Vibe check**: Would you trust this wallet with your money based on the setup experience?

```
Rating (Trust): ___/5
Notes:


```

---

### Mission 4.2 — The Wallet Dashboard

**Goal**: Explore the wallet panel. Does the layout make sense?

- [ ] **Sidebar**: 5 tabs visible? (Dashboard, Activity, Certificates, Approved Sites, Settings)
- [ ] Click each sidebar tab — does content change smoothly?
- [ ] **Dashboard tab**: Balance displayed? QR code for receiving? Send form? Recent activity?
- [ ] **Activity tab**: Transaction list? Can you filter sent/received? Pagination?
- [ ] **Certificates tab**: Empty state message if none?
- [ ] **Approved Sites tab**: Domain permission list?
- [ ] **Settings tab**: Display name, mnemonic reveal, backup, delete?

**Side quest**: Resize the browser window. Does the wallet panel adapt? Any overflow or cutoff?

```
Notes:


```

---

### Mission 4.3 — Receive BSV

**Goal**: Get some satoshis into your wallet. (Requires a second wallet or faucet.)

- [ ] Copy your receive address from the dashboard
- [ ] QR code present? Does it scan with a mobile wallet?
- [ ] Send a small amount from another wallet to this address
- [ ] Does the balance update? How quickly?
- [ ] Does the transaction appear in the Activity tab?
- [ ] Does the transaction show the correct amount and direction?

```
Notes:


```

---

### Mission 4.4 — Send BSV

**Goal**: Send satoshis to someone. The most critical wallet flow.

- [ ] On the Dashboard, find the send form
- [ ] Enter a valid BSV address and amount
- [ ] Click Send — what feedback do you get?
- [ ] Does the balance update after sending?
- [ ] Does the sent transaction appear in Activity?
- [ ] Try sending to an invalid address — is there a clear error?
- [ ] Try sending more than your balance — is there a clear error?
- [ ] Try sending 0 — is there validation?

**Watch for**: Is there a confirmation step before sending? Loading state during broadcast? Success/failure feedback?

```
Rating (Send Flow): ___/5
Notes:


```

---

### Mission 4.5 — Send via Paymail

**Goal**: Test the human-readable payment addresses.

- [ ] In the send form, type a paymail address (e.g., `someone@handcash.io` or `$handle`)
- [ ] Does the recipient resolve? (Name, avatar, checkmark)
- [ ] Send a small amount — does it go through?
- [ ] Try an invalid paymail — is there a clear "not found" message?
- [ ] Try a `$handle` (HandCash format) — does it resolve?

```
Notes:


```

---

### Mission 4.6 — Send via Identity Key (PeerPay)

**Goal**: Test the BRC-29 PeerPay flow.

- [ ] In the send form, paste a 66-character hex identity key
- [ ] Does it detect the format? Show "Identity key detected"?
- [ ] If the identity resolves (overlay services lookup), do you see a name?
- [ ] Send a small amount — does it go through via PeerPay?
- [ ] Check the receiving wallet — does the payment arrive?

```
Notes:


```

---

## Tier 5: Wallet — Advanced & Settings

*You've got the basics. Now push the wallet harder.*

---

### Mission 5.1 — Wallet Recovery

**Goal**: Recover a wallet from a mnemonic phrase.

- [ ] If you have an existing wallet, delete it first (Settings tab → Delete Wallet)
- [ ] Does deletion require confirmation? Does it refuse if you have a balance?
- [ ] Click wallet icon → "Recover Wallet"
- [ ] Enter your 12-word mnemonic
- [ ] Set a new PIN
- [ ] Does the wallet load with the correct balance and history?

**Stress test**: Enter a mnemonic with typos — is there useful error messaging?

```
Notes:


```

---

### Mission 5.2 — Backup & Export

**Goal**: Make sure backup works before you need it.

- [ ] Go to wallet Settings tab
- [ ] Click "Export Backup" — does a file save?
- [ ] What format is it? Can you open and inspect it?
- [ ] "Reveal Mnemonic" — does it require your PIN?
- [ ] After entering PIN, are the 12 words shown?

```
Notes:


```

---

### Mission 5.3 — PIN & Security

**Goal**: Test the PIN lock/unlock cycle.

- [ ] Close the wallet panel
- [ ] Reopen it — are you prompted for a PIN? Or is it auto-unlocked?
- [ ] If auto-unlock: close the entire browser, reopen, open wallet. Now is there a PIN prompt?
- [ ] Enter wrong PIN — is there a clear error?
- [ ] Enter correct PIN — does it unlock smoothly?

**Note**: On Windows, DPAPI should auto-unlock the wallet between sessions (no PIN needed). On macOS, Keychain does the same. If auto-unlock fails, note the platform.

```
Platform tested:
Auto-unlock works: [ ] Yes [ ] No
Notes:


```

---

### Mission 5.4 — Wallet Settings

**Goal**: Test every setting in the wallet settings tab.

- [ ] Change display name — does it persist?
- [ ] Default per-transaction limit — change it, verify it applies to new site approvals
- [ ] Default per-session limit — same
- [ ] Rate limit — same
- [ ] Do these defaults show up correctly in the Approved Sites tab for new domains?

```
Notes:


```

---

### Mission 5.5 — Domain Permissions (Approved Sites)

**Goal**: Test the spending approval system.

- [ ] Visit a BRC-100 enabled site (if available)
- [ ] Does a permission prompt appear asking to approve the domain?
- [ ] Approve it — does it appear in the Approved Sites list?
- [ ] Edit the spending limits for that domain
- [ ] Revoke the permission — can you re-add it?
- [ ] "Reset All" button — does it clear everything?

```
Notes:


```

---

## Tier 6: Web3 Identity (BRC-100)

*This is the unique value proposition. Test authentication with BSV identity sites.*

---

### Mission 6.1 — BRC-100 Authentication

**Goal**: Authenticate with a BRC-100 enabled site.

- [ ] Visit a BRC-100 site (Babbage-ecosystem site, if available)
- [ ] Does the authentication prompt appear?
- [ ] Approve — does authentication succeed?
- [ ] Is your identity (certificates, permissions) recognized?
- [ ] After authenticating, does the site function correctly?

**Note**: BRC-100 sites are limited in the wild. If none are available, note that and skip.

```
Sites tested:
Notes:


```

---

### Mission 6.2 — Certificate Management

**Goal**: Check that BRC-52 certificates are tracked.

- [ ] After authenticating with a BRC-100 site, check the Certificates tab
- [ ] Are certificates listed?
- [ ] Can you expand certificate details?
- [ ] What information is shown? Is it understandable?

```
Notes:


```

---

## Tier 7: Break Things

*You've been nice. Now be mean. Try to break the browser.*

---

### Mission 7.1 — Rapid Actions

- [ ] Open/close 20 tabs in rapid succession
- [ ] Open/close the wallet panel 10 times quickly
- [ ] Click multiple toolbar buttons rapidly
- [ ] Type in the address bar while a page is loading
- [ ] Navigate back/forward rapidly on a deep browsing session

```
Crashes: [ ] Yes [ ] No
Freezes: [ ] Yes [ ] No
Notes:


```

---

### Mission 7.2 — Edge Cases

- [ ] Visit `about:blank` — does it load?
- [ ] Type garbage in the address bar — does it search or show error?
- [ ] Visit a site with a self-signed SSL certificate — what happens?
- [ ] Visit a site that takes forever to load — is there a loading indicator?
- [ ] Visit a page with a `beforeunload` dialog ("Are you sure you want to leave?")
- [ ] Open DevTools (F12) and poke around — does the console work?
- [ ] Try to download a very large file (1GB+) — any issues?

```
Notes:


```

---

### Mission 7.3 — Offline & Network

- [ ] Disconnect WiFi/network while browsing — what happens?
- [ ] With network off, try to send BSV — is there a clear error?
- [ ] Reconnect — does browsing resume normally?
- [ ] During a page load, disconnect network — does it timeout gracefully?

```
Notes:


```

---

### Mission 7.4 — Window Stress

- [ ] Open 5+ windows with multiple tabs each
- [ ] Minimize all, then restore all
- [ ] Close windows in random order
- [ ] Resize a window very small — does content handle it?
- [ ] Maximize and restore — any rendering glitches?

```
Notes:


```

---

## Bug & Feels Journal

*Use this section for anything that doesn't fit above. Stream of consciousness is fine.*

### Session Log

| Date | Tester | Platform | Time Spent | Tiers Covered |
|------|--------|----------|------------|---------------|
| | | | | |
| | | | | |

### Bugs Found

| # | Severity | Where | Description | Steps to Reproduce |
|---|----------|-------|-------------|-------------------|
| 1 | | | | |
| 2 | | | | |
| 3 | | | | |

**Severity guide:**
- **Critical**: Crash, data loss, security issue, can't use core feature
- **Major**: Feature broken but workaround exists, significant UX issue
- **Minor**: Cosmetic, typo, slight inconvenience
- **Wish**: Not a bug, but would be nice

### General UX Notes

```
What felt good:


What felt bad:


What confused you:


What's missing:


Would you use this as your daily browser? Why/why not:


```

---

## Tester Quick Reference

### How to Launch (Windows)

```
Terminal 1: cd frontend && npm run dev        (leave running)
Terminal 2: cd cef-native/build/bin/Release && ./HodosBrowserShell.exe
```

Wallet (port 3301) and adblock engine (port 3302) auto-launch with the browser. Run them manually in separate terminals if you want to see their logs.

### How to Launch (macOS — when ported)

```
Terminal 1: cd frontend && npm run dev        (leave running)
Terminal 2: cd cef-native/build/bin/Release && ./HodosBrowserShell
```

### Key Ports

| Service | Port | Purpose |
|---------|------|---------|
| Frontend | 5137 | React UI |
| Wallet | 3301 | Bitcoin wallet backend |
| Adblock | 3302 | Ad/tracker blocking engine |

### Reporting Bugs

Include:
1. What you were doing
2. What you expected
3. What actually happened
4. Screenshot if visual
5. Console errors if available (F12 → Console tab)

---

*Last updated: 2026-03-09*
