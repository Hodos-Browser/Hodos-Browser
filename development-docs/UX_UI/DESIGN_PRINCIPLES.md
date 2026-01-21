# Web3 BitcoinSV UI/UX Design Principles & Philosophy

## Purpose

This document establishes the foundational design principles, philosophy, and interaction rules for Web3 BitcoinSV interfaces in Hodos Browser. These principles guide all UI/UX implementation decisions.

**Document Version:** 1.0
**Last Updated:** 2026-01-27
**Target Audience:** UI/UX Designers, Frontend Developers, Product Team

---

## Core Design Philosophy

### Hodos Browser UX Philosophy

**Security-first without friction**
- Protect the user by default; never silently approve identity or payment actions.

**Clean + simple UI**
- Minimal screens, minimal text, no clutter.

**Human-readable by default**
- Show friendly values (e.g., dollars + rounded sats), with "Show details" for raw data.

**Always give feedback**
- Every action must show immediate response: pressed state → loading → success/fail.

**Progressive disclosure**
- Keep defaults simple; advanced details are available but hidden.

**Non-annoying permission model**
- Avoid repeated prompts. Use quiet indicators for routine requests; interrupt only for sensitive actions.

**Consistent + predictable interactions**
- Same action looks/behaves the same everywhere.

**Calm, confident tone**
- Reduce fear; build trust through clarity.

---

## Design Principles

### 1. Security / Privacy Without Friction

**Principle**: Security prompts must be rare, meaningful, and user-controlled.

**Rules**:
- Never silently approve sensitive actions (PII / payments / identity disclosures)
- Default posture: minimize data sharing
- Security prompts should feel like "the browser is protecting you" not "the wallet is nagging you"

**Application**:
- Default sensitive permissions to "Ask Every Time"
- Provide "Allow Once" and "Always Allow" options
- Provide "Block This Site" option for repeated abuse/spam
- Show meaningful context (what site is requesting, what it wants, what it can do)

### 2. Clean + Simple

**Principle**: Prefer fewer screens and fewer options.

**Rules**:
- Avoid technical terms unless necessary (but allow "advanced details" reveal)
- Minimize screens and options
- Remove clutter and unnecessary elements

**Application**:
- Progressive disclosure: default view is simple, advanced details are collapsed
- Use "Show details" sections for raw data (pubkey, certificate payload, txid, sat amounts)
- Keep primary actions prominent, secondary actions subtle

### 3. Human-Readable

**Principle**: Present information in terms users understand.

**Rules**:
- Show "$0.02" instead of "0.00003124 BSV"
- Default to rounded/short numbers but allow "copy exact value" + "view full details"
- Use clear, friendly language

**Application**:
- Display currency in human-friendly format (dollars, rounded sats)
- Provide exact values on demand
- Avoid raw technical data in default views
- Show "Why is this needed?" links for certificates/PII

### 4. Immediate Feedback

**Principle**: Every click produces a visible response within 100ms.

**Rules**:
- Button press state
- Subtle highlight
- Loading spinner
- Toast confirmation
- All requests must have: In Progress → Success/Fail → Next Action

**Application**:
- Visual feedback for all interactions
- Loading states during async operations
- Success/error messages after operations complete
- Clear next actions after completion

### 5. No Ambiguity

**Principle**: UI must never leave users wondering.

**Key Questions to Answer**:
- "Did it work?"
- "Where did my money go?"
- "Did I approve something dangerous?"

**Application**:
- Clear success/error states
- Transaction confirmations with details
- Permission approvals with context
- Clear next actions

### 6. Non-Annoying Permission Model

**Principle**: Don't interrupt browsing unless it's sensitive.

**Rules**:
- Prompts should feel like: "the browser is protecting you" not "the wallet is nagging you"
- Reduce prompt fatigue (key for BRC-100)
- If site requests pubkey on load:
  - Do NOT show a full modal every time
  - Instead: quiet indicator + queue notifications
- Escalate only for:
  - Certificates / identifiers (PII)
  - Payments
  - First-time request from a site

**Application**:
- Use quiet indicators for routine requests
- Interrupt only for sensitive actions
- Allow pre-approval for trusted sites
- Show permission status without blocking

### 7. Consistency

**Principle**: Same actions look and behave the same everywhere.

**Rules**:
- Same terminology everywhere (connect / approve / deny / always allow / this time only)
- Same visual language across interfaces
- Predictable behavior patterns

**Application**:
- Consistent button styles and placement
- Consistent terminology across all interfaces
- Consistent permission flows
- Consistent error handling

### 8. Calm Confidence

**Principle**: Tone: professional, calm, minimal.

**Rules**:
- No scary language unless truly necessary (payments and identity)
- Build trust through clarity
- Reduce fear, increase confidence

**Application**:
- Professional, friendly language
- Clear, confident messaging
- Warning language only for truly risky actions
- Trust indicators and context

---

## Interaction Rules (Micro UX Requirements)

### Buttons

**Required States**:
- ✅ Hover state
- ✅ Pressed state
- ✅ Disabled state
- ✅ Loading state (spinner inside button)

**Implementation**:
```typescript
// Example button states
<Button
  disabled={loading}
  sx={{
    '&:hover': { /* hover styles */ },
    '&:active': { /* pressed styles */ },
    '&:disabled': { /* disabled styles */ }
  }}
>
  {loading ? <CircularProgress size={20} /> : 'Action'}
</Button>
```

### Inputs

**Required Behavior**:
- ✅ Real-time validation
- ✅ Error text under the field (not alerts)
- ✅ Clear error states

**Implementation**:
- Validate on blur and change
- Show inline error messages
- Use helper text for guidance
- Never show errors in alerts/modals

### Copy Actions

**Required Behavior**:
- ✅ Always show "Copied ✓" feedback
- ✅ Visual confirmation (icon change or toast)

**Implementation**:
```typescript
// Copy button with feedback
const [copied, setCopied] = useState(false);
const handleCopy = () => {
  navigator.clipboard.writeText(text);
  setCopied(true);
  setTimeout(() => setCopied(false), 2000);
};
// Show "Copied ✓" or icon change
```

### Modals

**Required Behavior**:
- ✅ Clear close button
- ✅ Escape key support
- ✅ Don't trap user unless necessary

**Implementation**:
- Always provide visible close button (X icon)
- Support Escape key to close
- Make close button accessible
- Only trap focus for critical confirmations

### Long Actions

**Required Behavior**:
- ✅ Show progress: "Broadcasting transaction…" not just "Loading…"
- ✅ Specific status messages

**Implementation**:
- Use descriptive loading text
- Show progress indicators when possible
- Update status messages during long operations
- Example: "Broadcasting transaction…", "Confirming…", "Processing…"

### Errors

**Required Elements**:
- ✅ Explain what happened
- ✅ What the user can do next
- ✅ Never display raw HTTP errors to users by default

**Error Style Rules**:
- Every error must explain:
  - What happened
  - What the user can do next
- Recommended next actions:
  - "Try again"
  - "View details"
  - "Block site"
- Never show raw HTTP/technical errors by default
- Use friendly error messages

**Implementation**:
```typescript
// Good error message
"Failed to connect to wallet server.
Please check your connection and try again."
[Try Again] [View Details]

// Bad error message
"Error 500: Internal Server Error"
```

---

## Progressive Disclosure

**Principle**: Reveal complexity progressively.

**Default View**: Simple
- Show only essential information
- Use human-readable formats
- Hide technical details

**Advanced Details**: Collapsed section ("Show details")
- Raw pubkey
- Certificate payload
- Transaction ID (txid)
- Exact sat amounts
- Technical metadata

**Implementation**:
- Use expandable sections with "Show details" / "Hide details"
- Default state: collapsed (hidden)
- Allow users to reveal complexity when needed
- Keep default view clean and simple

---

## Fast Escape Hatches

**Principle**: Every modal has clear exit options.

**Required**:
- ✅ Clear close button (visible, accessible)
- ✅ Escape key support
- ✅ Don't trap user unless necessary

**Exceptions**:
- Critical confirmations (payment, identity disclosure) may require explicit action
- Always provide a way out, even if it means canceling the action

---

## Accessibility (MVP-Level Only)

**Minimum Requirements**:
- ✅ Minimum 14px text
- ✅ Good contrast (WCAG AA minimum)
- ✅ Full keyboard navigation for wallet modal + approvals
- ✅ Screen reader support for critical actions

**Implementation**:
- Ensure all interactive elements are keyboard accessible
- Use proper ARIA labels
- Maintain minimum font sizes
- Test with keyboard-only navigation

---

## Trust and Transparency

**Principle**: Always show context and allow user control.

**Required Information**:
- ✅ What site is requesting
- ✅ What it wants
- ✅ What it can do
- ✅ Show a "Why is this needed?" link for certificates/PII

**Implementation**:
- Always display requesting domain prominently
- Explain the purpose of the request
- Show what permissions/data will be shared
- Provide educational links when appropriate
- Use clear, honest language

---

## Default Safety Behaviors

**Principle**: Secure by default, convenient when appropriate.

**Default Settings**:
- ✅ Default sensitive permissions to "Ask Every Time"
- ✅ Provide "Allow Once" option
- ✅ Provide "Always Allow" option (user choice)
- ✅ Provide "Block This Site" option for repeated abuse/spam

**Implementation**:
- Never auto-approve sensitive actions
- Always require explicit user consent
- Allow users to set preferences
- Remember user preferences for trusted sites
- Easy revocation of permissions

---

## Visual Feedback Timing

**Principle**: Immediate response to user actions.

**Timing Requirements**:
- ✅ Visual feedback within 100ms of click/interaction
- ✅ Loading states during async operations
- ✅ Success/error feedback after completion
- ✅ Status updates during long operations

**Implementation**:
- Optimistic UI updates when safe
- Immediate visual feedback (button press, highlight)
- Loading spinners during async operations
- Toast notifications for success/errors
- Progress indicators for long operations

---

## Permission Model Escalation

**Principle**: Interrupt only when necessary.

**Escalation Levels**:

1. **Quiet Indicator** (No interruption)
   - Routine requests from trusted sites
   - Public key requests (non-PII)
   - Low-risk operations
   - Background activity monitoring

2. **Notification** (Minimal interruption)
   - First-time site requests
   - Routine operations from new sites
   - Informational only (user can dismiss)

3. **Modal Prompt** (Requires action)
   - Certificates / identifiers (PII)
   - Payments / transactions
   - Identity disclosures
   - High-risk operations

**Application**:
- Use quiet indicators for routine operations
- Show notifications for new sites
- Use modals only for sensitive actions
- Allow pre-approval for trusted sites

---

## Usage

These principles will be referenced in:
- [Initial Setup/Recovery Implementation](./INITIAL_SETUP_RECOVERY.md)
- [User Notifications Implementation](./USER_NOTIFICATIONS.md)
- [Light Wallet Implementation](./LIGHT_WALLET.md)
- [Full Wallet Implementation](./FULL_WALLET.md)
- [Activity Status Indicator Implementation](./ACTIVITY_STATUS_INDICATOR.md)

---

## Summary

**Key Takeaways**:
1. **Security without friction**: Protect users without annoying them
2. **Clean + simple**: Minimal screens, minimal options
3. **Human-readable**: Friendly values, technical details on demand
4. **Immediate feedback**: Visual response within 100ms
5. **No ambiguity**: Always answer "did it work?"
6. **Non-annoying permissions**: Interrupt only for sensitive actions
7. **Consistency**: Same actions behave the same everywhere
8. **Calm confidence**: Professional tone, build trust

**Core Philosophy**:
Security-first, user-friendly, clean, simple, transparent, and trustworthy.

---

**End of Document**
