# Checkpoint: Transaction Error Handling & UI Feedback

> **Date**: 2025-12-02
> **Status**: Critical Issue - Needs Immediate Fix
> **Context**: Between Phase 4 (UTXO Management) and Phase 5 (BEEF/SPV Caching)

---

## 🚨 **Critical Issue: Transaction Failure Not Reported to User**

### **Problem Summary**

When a transaction broadcast fails, the wallet backend correctly:
- ✅ Logs the error in console
- ✅ Updates transaction status to "failed" in database
- ❌ **BUT**: Returns success status to frontend
- ❌ **Frontend shows "Transaction Sent!" even though it failed**

### **Error Details**

**Backend Log:**
```
[2025-12-02T00:00:49Z WARN  hodos_wallet::handlers]    ⚠️ WhatsOnChain failed: 400 Bad Request - "unexpected response code 500: 16: mandatory-script-verify-flag-failed (Script failed an OP_EQUALVERIFY operation)"
```

**Root Cause Analysis:**
1. **Script Verification Error**: The transaction script failed validation (`OP_EQUALVERIFY` operation failed)
   - Possible causes:
     - Incorrect script generation
     - Invalid signature
     - Script mismatch between what we built and what we're signing
     - BRC-29 derivation issue (if it was a BRC-29 payment)

2. **Broadcast Logic Issue**: The `broadcast_transaction` function tries multiple broadcasters (GorillaPool, WhatsOnChain)
   - If one succeeds, it returns success
   - If both fail, it returns error
   - **BUT**: The `processAction` handler may not be properly propagating the error status

### **Current Code Flow**

**File**: `rust-wallet/src/handlers.rs` (lines 3265-3315)

```rust
match broadcast_transaction(&raw_tx).await {
    Ok(_) => {
        // Updates status to "unconfirmed" ✅
        status = "completed"
    }
    Err(e) => {
        // Updates status to "failed" ✅
        // BUT: status = "failed" ✅
        status = "failed"
    }
}

HttpResponse::Ok().json(ProcessActionResponse {
    txid,
    status: status.to_string(),  // Should be "failed" but frontend might not handle it
    raw_tx: Some(raw_tx),
})
```

**Issue**: The response always returns HTTP 200 OK, even when status is "failed". The frontend needs to check the `status` field, but it might be showing success based on HTTP status code.

### **Frontend Handling**

**Files to Check:**
- `frontend/src/components/panels/WalletPanelContent.tsx` (lines 280-321)
- `frontend/src/components/TransactionForm.tsx`
- `frontend/src/hooks/useTransaction.ts` (if exists)

**Current Behavior:**
- Frontend likely checks HTTP status code (200 = success)
- Should check `response.status` field instead
- Need to display error message when `status === "failed"`

---

## 📋 **Action Items**

### **1. Fix Backend Error Propagation** (Priority: HIGH)
- [ ] Ensure `processAction` returns appropriate HTTP status codes
  - 200 OK for success
  - 400/500 for failures
- [ ] OR: Keep 200 OK but ensure `status` field is always accurate
- [ ] Add detailed error messages in response body

### **2. Fix Frontend Error Handling** (Priority: HIGH)
- [ ] Update frontend to check `response.status` field, not just HTTP status
- [ ] Display error message when transaction fails
- [ ] Show user-friendly error messages
- [ ] Don't show "Transaction Sent!" success message if status is "failed"

### **3. Investigate Script Verification Error** (Priority: MEDIUM)
- [ ] Debug why `OP_EQUALVERIFY` is failing
- [ ] Check BRC-29 derivation logic (if applicable)
- [ ] Verify script generation matches signing
- [ ] Test with simple P2PKH transaction first

### **4. Improve Error Logging** (Priority: LOW)
- [ ] Add more detailed error context
- [ ] Log full transaction hex for debugging
- [ ] Log which broadcaster failed and why

---

## 🔍 **Investigation Notes**

### **Script Verification Error Analysis**

The error `mandatory-script-verify-flag-failed (Script failed an OP_EQUALVERIFY operation)` suggests:

1. **P2PKH Script Issue**: Standard P2PKH scripts use `OP_EQUALVERIFY` to verify the public key hash
   ```
   OP_DUP OP_HASH160 <pubkeyhash> OP_EQUALVERIFY OP_CHECKSIG
   ```

2. **Possible Causes**:
   - Wrong public key hash in locking script
   - Wrong public key in unlocking script
   - Signature doesn't match public key
   - BRC-29 derivation produced wrong address/script

3. **Debugging Steps**:
   - Log the locking script we're creating
   - Log the unlocking script we're generating
   - Verify the derived public key matches the address
   - Check BRC-29 invoice number format

---

## 📝 **Related Files**

**Backend:**
- `rust-wallet/src/handlers.rs` - `processAction` handler (lines 3190-3316)
- `rust-wallet/src/handlers.rs` - `broadcast_transaction` function (lines 3318-3357)
- `rust-wallet/src/handlers.rs` - `signAction` handler (BRC-29 derivation)

**Frontend:**
- `frontend/src/components/panels/WalletPanelContent.tsx` - Success modal
- `frontend/src/components/TransactionForm.tsx` - Transaction submission
- `frontend/src/hooks/useTransaction.ts` - Transaction hook (if exists)

**CEF Native:**
- `cef-native/src/handlers/simple_handler.cpp` - Broadcast transaction handler
- `cef-native/src/handlers/simple_render_process_handler.cpp` - Response forwarding

---

## 🎯 **Success Criteria**

- [ ] Failed transactions show error message to user
- [ ] Success message only shown when transaction actually succeeds
- [ ] Error messages are user-friendly and actionable
- [ ] Script verification errors are properly logged and debugged
- [ ] Transaction status accurately reflects reality

---

## ⚠️ **Additional Notes**

### **Performance Issue**
- Wallet is still slow - fetching everything on every balance check
- Need to discuss optimization strategy (caching, background sync, gap limit)
- See Phase 4 documentation for details

### **Phase 4 Status**
- ✅ UTXO caching implemented
- ✅ Balance calculation from database
- ✅ UTXO spending tracking
- ⏳ Background sync with gap limit (pending)
- ⏳ Periodic UTXO updates (pending)

---

**Next Session**: Fix transaction error handling and UI feedback before continuing with Phase 5.
