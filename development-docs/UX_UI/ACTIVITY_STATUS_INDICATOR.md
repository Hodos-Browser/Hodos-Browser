# Activity Status Indicator Implementation Plan

## Overview

**Interface Type**: Passive UI Element
**Purpose**: Display passive indicator of background wallet/permission activity, allowing users to monitor activity and review/change permissions without constant notifications

**Status**: 📋 Planning Phase (Low Priority - Move to End)
**Last Updated**: 2026-01-27

---

## Interface Description

The Activity Status Indicator is a passive UI element that:
- Shows background activity happening (payments, data requests, permissions being used)
- Provides summary/overview of activity (not individual items)
- Allows users to click to review recent activity and permissions
- Enables users to change permissions if they see concerning activity
- Non-intrusive - respects user privacy by not constantly showing notifications

**Key Design Philosophy**:
- **Privacy & Security First**: Protect user privacy and security
- **Minimal Interruption**: Don't bombard users with notifications for trusted sites
- **Informed Choice**: Give users visibility into what's happening so they can make decisions
- **Default Approvals**: Pre-approved defaults for trusted sites reduce notification noise

**Display Location**:
- Browser header/toolbar (recommended)
- System tray/notification area (alternative)
- Status bar (alternative)

---

## Requirements

### Functional Requirements
- [ ] Display activity summary/indicator (not individual items)
- [ ] Visual indicator (badge, icon, subtle animation)
- [ ] Click to open activity/permission review dashboard
- [ ] Show activity metrics (recent payments, requests, etc.)
- [ ] Allow permission changes from review interface
- [ ] Respect whitelist/trust settings (only show summary, not interruptions)
- [ ] Different visual states (idle, active, concerning activity)

### Non-Functional Requirements
- [ ] Always visible (persistent passive indicator)
- [ ] Minimal UI footprint
- [ ] Non-intrusive (never interrupts user)
- [ ] Accessible (screen reader support)
- [ ] Clear but subtle visual feedback
- [ ] Performance optimized (doesn't slow down browser)

---

## Frontend Implementation

### Component Structure

**Location**: `frontend/src/components/ActivityStatusIndicator.tsx` (or similar)

**Type**: React Functional Component

**Props**:
```typescript
interface ActivityStatusIndicatorProps {
  activitySummary: ActivitySummary; // Summary stats, not queue
  onClick: () => void; // Open activity review dashboard
  className?: string; // For styling/positioning
}

interface ActivitySummary {
  recentActivityCount: number; // Recent activity (last hour/day)
  recentPayments: number;
  recentDataRequests: number;
  permissionChangesAvailable: boolean;
  status: 'idle' | 'active' | 'high_activity';
}
```

**Component Structure**:
```typescript
<ActivityStatusIndicator
  activitySummary={activitySummary}
  onClick={handleOpenActivityDashboard}
  className="header-indicator"
/>
```

**Visual States**:
1. **Hidden/Minimal** - No recent activity (everything normal)
2. **Idle** - Normal background activity (trusted sites, pre-approved)
3. **Active** - Moderate activity happening
4. **Alert** - Concerning activity detected (unusual patterns, high volume)

**Visual Design**:
- Subtle icon/badge (e.g., activity icon, wallet icon)
- Optional small count/metric (recent activity count)
- Color coding (green = normal, yellow = active, red = alert)
- Subtle pulse/animation only on state changes (not constant)

---

## CEF-Native Implementation

### Activity Tracking

**Activity Monitoring**:
- Track background activity (payments, requests, permissions)
- Aggregate activity into summary (not individual queue items)
- Monitor for unusual patterns
- Respect whitelist/trust settings

**Activity Tracking Structure**:
```cpp
struct ActivitySummary {
    int recentActivityCount; // Last hour/day
    int recentPayments;
    int recentDataRequests;
    bool hasConcerningActivity; // Unusual patterns
    time_t lastUpdate;
};
```

**Activity Operations**:
- Track activity events (payments, requests, permissions)
- Aggregate into time-based summaries
- Detect unusual patterns
- Update indicator when summary changes

### Message Handling

**Activity Messages**:
- `activity_track_event` - Track a new activity event
- `activity_get_summary` - Get current activity summary
- `activity_open_dashboard` - Open activity review dashboard
- `activity_update_indicator` - Update indicator display

**Status Updates**:
- Real-time aggregation of activity
- Periodic summary updates (every minute or on threshold)
- Event-driven updates (immediate on concerning activity)

---

## Rust Wallet Backend

### Activity Tracking (If Needed)

**Database Schema** (potential):
**Activity Log Table**:
- Event ID
- Event type (payment, request, permission)
- Source domain
- Details (JSON)
- Timestamp
- Status (approved, auto-approved, denied)

**Activity Summary Cache**:
- In-memory or cached aggregated summaries
- Time-windowed aggregation (hourly, daily)

**Decision**: Do we need persistent activity log (for history) or real-time summary only?

### API Endpoints (If Needed)

**Potential Endpoints**:
- `GET /activity/summary` - Get current activity summary
- `GET /activity/recent` - Get recent activity items (for dashboard)
- `GET /activity/permissions` - Get current permission settings
- `POST /activity/permissions/update` - Update permission settings

**Note**: Activity tracking might primarily happen in C++ (interceptor level), with Rust providing API access.

---

## Database Considerations

### Current Schema

- Domain whitelist table exists
- No activity log table exists

### Potential Additions

**Activity Log Table** (if history needed):
- Event ID (primary key)
- Event type
- Source domain
- Event details (JSON blob)
- Timestamp
- Auto-approved flag (was this pre-approved?)

**Activity Aggregates Table** (for performance):
- Time window (hour, day)
- Activity counts
- Payment counts
- Request counts
- Pre-computed summaries

**Decision Points**:
1. Do we need persistent activity history?
2. How long to retain activity data?
3. Should activity be per-domain aggregated or global?

---

## Triggers

### Activity Tracking Triggers

1. **HTTP Interceptor** (Payments, requests)
   - Location: `cef-native/src/core/HttpRequestInterceptor.cpp`
   - When: Payment sent, data requested
   - Action: Track activity, update summary

2. **Wallet Operations** (Transactions)
   - Location: Transaction sending flow
   - When: Transaction completed
   - Action: Track payment activity

3. **Permission Usage** (BRC-100, identity)
   - Location: Permission handlers
   - When: Permission used (even if pre-approved)
   - Action: Track permission activity

4. **Background Sync** (UTXO, balance)
   - Location: Background sync services
   - When: Sync operations complete
   - Action: Update activity summary

### Summary Update Triggers

- Activity event occurs → Aggregate into summary
- Time-based update (periodic refresh)
- Threshold reached (e.g., X events in Y time)
- Unusual pattern detected → Alert state

---

## User Interaction Flow

### Passive Monitoring Flow

```
1. Background activity occurs (payment, request, etc.)
   ↓
2. Activity tracked and aggregated into summary
   ↓
3. Summary updated
   ↓
4. Indicator reflects current activity state
   ↓
5. User sees subtle indicator (no interruption)
   ↓
6. User can click when they want to review
```

### Activity Review Flow

```
1. User clicks Activity Status Indicator
   ↓
2. Activity Review Dashboard opens (modal, panel, or page)
   ↓
3. Dashboard shows:
   - Recent activity summary
   - Recent payments
   - Recent permission requests
   - Current permission settings
   ↓
4. User reviews activity
   ↓
5. User can:
   - See details of specific activities
   - Change permission settings
   - Whitelist/trust new sites
   - Revoke permissions
   - View activity history
   ↓
6. User closes dashboard (no forced actions)
```

---

## Design Considerations

**Reference**: [Design Principles](./DESIGN_PRINCIPLES.md)

Key considerations:
- [ ] **Privacy First**: Don't expose activity details unless user wants to see
- [ ] **Minimal Interruption**: Indicator is passive, never blocks user
- [ ] **Informed Control**: Give visibility without annoyance
- [ ] **Trust Defaults**: Pre-approved trusted sites don't generate alerts
- [ ] **Concerning Activity**: Detect and highlight unusual patterns
- [ ] **Visual Subtlety**: Indicator should be noticeable but not distracting
- [ ] **Accessibility**: Screen reader support for activity summaries

---

## Testing Requirements

### Unit Tests
- Component rendering
- Activity summary aggregation
- State transitions
- Click handling

### Integration Tests
- Activity tracking integration
- Summary updates
- Dashboard opening
- Permission management integration

### User Acceptance Tests
- Visibility (does user notice it when needed?)
- Subtlety (does it stay out of way normally?)
- Usefulness (is activity summary helpful?)
- Actionability (can user effectively review/change permissions?)

---

## Dependencies

### External Dependencies
- HTTP Interceptor (for activity tracking)
- Wallet operations (for payment tracking)
- Permission system (for permission tracking)
- Whitelist/trust system (for pre-approval logic)

### Internal Dependencies
- Activity aggregation logic
- Activity review dashboard (separate component)
- Permission management UI

---

## Related Documentation

- [User Notifications](./USER_NOTIFICATIONS.md) - Notification system (different - active notifications)
- [HTTP Interceptor Flow Guide](./HTTP_INTERCEPTOR_FLOW_GUIDE.md) - Request interception
- [Design Principles](./DESIGN_PRINCIPLES.md) - Design guidelines
- [UX Design Considerations](./UX_DESIGN_CONSIDERATIONS.md) - UX patterns

---

## Open Questions

1. What constitutes "concerning activity" that should trigger alert state?
2. How detailed should activity summary be (just counts, or more info)?
3. Do we need persistent activity history or real-time only?
4. What time window for "recent activity" (1 hour, 24 hours)?
5. Should indicator show different states for different activity types?
6. How to handle high-volume trusted sites (aggregate or detail)?
7. Should there be notification integration (active alerts vs passive indicator)?
8. What permissions should be reviewable/changable from dashboard?

---

## Implementation Notes

- **This is LOW PRIORITY** - Move to end of implementation sequence
- This is a passive monitoring system, not a queue of pending items
- Focus on privacy and minimal interruption
- Activity aggregation is key - we want summaries, not noise
- Consider integration with whitelist/trust system for pre-approval logic
- May need activity review dashboard component (separate from indicator)

---

**End of Document**
