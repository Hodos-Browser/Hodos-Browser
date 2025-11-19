# Go-Wallet-Toolbox Migration Plan

## 🎯 Executive Summary

**Objective:** Migrate from custom wallet implementation to BSV's official `go-wallet-toolbox` to achieve full BRC-100 compliance and fix critical authentication issues.

**Strategy:** Phased migration with **zero downtime** - implement toolbox alongside existing code, test thoroughly, then cutover.

**Timeline:** 3-4 weeks (2 weeks implementation + 1-2 weeks testing)

---

## ❓ **QUICK ANSWERS TO KEY QUESTIONS**

### **Q1: Can we test wallet creation without losing our wallet with coins?**

**YES! Three layers of protection:**

1. **Test Mode:** Test with empty wallet in `./test_wallet/` (real wallet untouched)
   ```bash
   WALLET_TEST_MODE=true go run main.go
   ```

2. **Automatic Backups:** Before migration, wallet is backed up automatically
   ```
   %APPDATA%/BabbageBrowser/backups/2025-10-09_14-30-00/wallet.json
   ```

3. **Parallel Operation:** Both JSON and SQLite work simultaneously
   ```bash
   STORAGE_MODE=json   # Use original wallet.json
   STORAGE_MODE=gorm   # Use new wallet.db
   ```

**Your wallet with coins is NEVER at risk!**

---

### **Q2: Can we migrate our current wallet to the new SQL database?**

**YES! Complete migration workflow:**

```
Step 1: Backup (automatic)          → wallet.json copied to backups/
Step 2: Dry run (read-only test)    → Verify wallet readable
Step 3: Migrate (copy data)         → Create wallet.db with your data
Step 4: Verify (compare)            → Ensure all 9 addresses match
Step 5: Test (GORM mode)            → Test with your real data
Step 6: Keep both (safety)          → wallet.json + wallet.db both exist
```

**Tool:** `safe_migrate.go` handles everything automatically

---

### **Q3: Should we put domain whitelist in SQL too?**

**YES! Benefits:**

| Feature | JSON File | SQLite |
|---------|-----------|--------|
| **Lookup Speed** | Load entire file | Instant indexed query |
| **Usage Stats** | None | Track request count, last used |
| **Thread Safety** | Manual locking | Database handles it |
| **Concurrent Access** | File locks | No issues |
| **Backup** | Separate file | Same database as wallet |

**Single database file:**
```
wallet.db
├── wallets table          (your wallet data)
├── addresses table        (your 9 addresses)
└── whitelisted_domains    (toolbsv.com, etc.)
```

**Migration:** Automatic in `safe_migrate.go` tool

---

### **Q4: Do we need to install SQL Server?**

**NO! SQLite is embedded:**

- ✅ No installation required
- ✅ Just a Go library
- ✅ Database is a single file (~100 KB)
- ✅ Works like JSON but more powerful
- ✅ **No C drive space concerns!**

**Space usage:**
```
Current:  wallet.json (5 KB) + domainWhitelist.json (1 KB) = 6 KB
New:      wallet.db (100 KB) = 100 KB
Additional: ~94 KB (practically nothing!)
```

---

### **Q5: What if something goes wrong?**

**Rollback is instant:**

```bash
# Switch back to JSON
STORAGE_MODE=json go run main.go

# Your wallet.json is still there!
# Your coins are safe!
```

**Zero data loss:**
- ✅ Original `wallet.json` never deleted
- ✅ Automatic backups before migration
- ✅ Can switch between JSON/SQLite anytime
- ✅ Verification ensures data matches

---

## 📊 Current State Analysis

### **Current Architecture:**
```
React UI (TypeScript)
    ↓ (CEF messages)
CEF C++ Bridge
    ↓ (HTTP localhost:3301)
Custom Go Daemon
    ↓
JSON File Storage (wallet.json, domainWhitelist.json)
    ↓
Bitcoin SV Network (via go-sdk)
```

### **Current Components:**

| Component | Implementation | Status |
|-----------|---------------|---------|
| **Storage** | JSON files (`wallet.json`) | ⚠️ Not production-ready |
| **Key Derivation** | Custom BRC-42 (P-256) | ❌ **BROKEN** (wrong curve) |
| **BRC-100 Compliance** | Partial (custom endpoints) | ⚠️ Incomplete |
| **Authentication** | Custom `/.well-known/auth` | ❌ Fails nonce verification |
| **BEEF Support** | Partial implementation | ⚠️ Incomplete |
| **Transaction System** | Custom (working) | ✅ Working |
| **CEF Integration** | Custom HTTP bridge | ✅ Working |
| **Frontend** | React (working) | ✅ Working |

### **Critical Issues to Fix:**
1. 🚨 BRC-42 key derivation uses P-256 instead of secp256k1
2. 🚨 Universal nonce verification failure across Babbage sites
3. 🚨 Missing `/brc100-auth` endpoint (ToolBSV compatibility)
4. ⚠️ JSON storage not production-ready
5. ⚠️ Incomplete BRC-100 standard compliance

---

## 🎯 Target Architecture

### **After Migration:**
```
React UI (TypeScript) - NO CHANGES
    ↓ (CEF messages)
CEF C++ Bridge - NO CHANGES
    ↓ (HTTP localhost:3301)
Hybrid Go Daemon (wrapper layer) - MODIFIED
    ↓
go-wallet-toolbox (BRC-100 wallet) - NEW
    ↓
GORM Storage (SQLite/Postgres) - NEW
    ↓
Bitcoin SV Network (via go-sdk)
```

### **Target Components:**

| Component | New Implementation | Benefit |
|-----------|-------------------|---------|
| **Storage** | GORM (SQLite initially) | ✅ Production-ready, queryable |
| **Key Derivation** | go-wallet-toolbox (secp256k1) | ✅ **Fixes critical bug** |
| **BRC-100 Compliance** | sdk.Interface implementation | ✅ Full standard compliance |
| **Authentication** | Standard BRC-100 auth | ✅ Compatible with all sites |
| **BEEF Support** | Complete BEEF workflows | ✅ Full SPV support |
| **Transaction System** | Hybrid (existing + toolbox) | ✅ Best of both |
| **CEF Integration** | **NO CHANGES** | ✅ Zero frontend impact |
| **Frontend** | **NO CHANGES** | ✅ Transparent migration |

---

## 📋 Migration Strategy Overview

### **Phase 1: Storage Migration (Week 1)**
- Migrate from JSON files to SQLite
- Run both systems in parallel
- Verify data integrity

### **Phase 2: Wallet Integration (Week 2)**
- Integrate go-wallet-toolbox wallet
- Implement BRC-100 standard endpoints
- Keep custom endpoints working

### **Phase 3: Testing & Validation (Week 3)**
- Test with ToolBSV, Babbage sites
- Performance testing
- Security validation

### **Phase 4: Cutover & Cleanup (Week 4)**
- Remove old JSON-based code
- Clean up dead code
- Documentation update

---

## 🔧 PHASE 1: Storage Migration (Isolated from Wallet Logic)

### **Goal:** Migrate from JSON files to SQLite WITHOUT changing wallet logic

### **Why Separate This?**
- ✅ Smaller, focused change
- ✅ Can test storage independently
- ✅ Reduces risk
- ✅ Can rollback easily

### **⚠️ CRITICAL: Wallet Data Protection**

**Your Current Situation:**
- ✅ `wallet.json` contains **real wallet with coins**
- ✅ `domainWhitelist.json` contains **trusted domains**
- 🚨 **MUST NOT LOSE THIS DATA**

**Protection Strategy:**
1. **Automatic backups** before any migration
2. **Test environment** separate from production wallet
3. **Verification** before deleting old files
4. **Parallel operation** (JSON + SQLite both work)

---

### **Step 0: Pre-Migration Safety (DO THIS FIRST!)**

#### **Step 0.1: Backup Existing Wallet**

**New File:** `go-wallet/backup_wallet.go`
```go
package main

import (
    "fmt"
    "io"
    "os"
    "path/filepath"
    "time"
)

// BackupWallet creates timestamped backup of wallet files
func BackupWallet() error {
    timestamp := time.Now().Format("2006-01-02_15-04-05")
    backupDir := filepath.Join(GetWalletDir(), "backups", timestamp)

    // Create backup directory
    if err := os.MkdirAll(backupDir, 0700); err != nil {
        return fmt.Errorf("failed to create backup dir: %v", err)
    }

    // Backup wallet.json
    if err := backupFile(GetWalletPath(), filepath.Join(backupDir, "wallet.json")); err != nil {
        return fmt.Errorf("failed to backup wallet.json: %v", err)
    }

    // Backup domainWhitelist.json
    whitelistPath := filepath.Join(GetWalletDir(), "domainWhitelist.json")
    if err := backupFile(whitelistPath, filepath.Join(backupDir, "domainWhitelist.json")); err != nil {
        // Non-fatal if whitelist doesn't exist
        log.Printf("Warning: Could not backup whitelist: %v", err)
    }

    log.Printf("✅ Wallet backed up to: %s", backupDir)
    log.Printf("   - wallet.json")
    log.Printf("   - domainWhitelist.json")

    return nil
}

func backupFile(src, dst string) error {
    srcFile, err := os.Open(src)
    if err != nil {
        return err
    }
    defer srcFile.Close()

    dstFile, err := os.Create(dst)
    if err != nil {
        return err
    }
    defer dstFile.Close()

    _, err = io.Copy(dstFile, srcFile)
    return err
}
```

**Usage:**
```go
func main() {
    // FIRST THING: Backup existing wallet
    if err := BackupWallet(); err != nil {
        log.Printf("⚠️  Backup failed: %v", err)
        // Continue anyway, but warn user
    }

    // Rest of main()...
}
```

**Backup Location:**
```
C:\Users\YourUser\AppData\Roaming\BabbageBrowser\backups\
    └── 2025-10-09_14-30-00\
        ├── wallet.json           (your real wallet with coins)
        └── domainWhitelist.json  (your trusted domains)
```

#### **Step 0.2: Create Test Wallet Environment**

**New File:** `go-wallet/test_mode.go`
```go
package main

import (
    "os"
    "path/filepath"
)

var (
    isTestMode = false
    testWalletPath string
)

// InitTestMode sets up isolated test environment
func InitTestMode() {
    // Check environment variable
    if os.Getenv("WALLET_TEST_MODE") == "true" {
        isTestMode = true

        // Use test directory instead of AppData
        testWalletPath = filepath.Join(".", "test_wallet")
        os.MkdirAll(testWalletPath, 0700)

        log.Println("🧪 TEST MODE: Using test wallet directory")
        log.Printf("   Path: %s", testWalletPath)
        log.Println("   ⚠️  Production wallet is SAFE")
    } else {
        log.Println("💰 PRODUCTION MODE: Using real wallet")
        log.Printf("   Path: %s", GetWalletPath())
    }
}

// GetWalletDir returns appropriate directory based on mode
func GetWalletDir() string {
    if isTestMode {
        return testWalletPath
    }
    return filepath.Join(os.Getenv("APPDATA"), "BabbageBrowser")
}
```

**Usage:**
```bash
# Test with temporary wallet (SAFE - doesn't touch real wallet)
WALLET_TEST_MODE=true go run main.go

# Production mode (uses real wallet)
go run main.go
```

---

### **Step 0.3: Migration Verification Tool**

**New File:** `go-wallet/verify_migration.go`
```go
package main

import (
    "encoding/json"
    "fmt"
)

// VerifyMigration compares JSON and GORM data for integrity
func VerifyMigration(jsonStorage *JSONStorage, gormStorage *GORMStorage) error {
    log.Println("🔍 Verifying migration integrity...")

    // Get data from both sources
    jsonWallet, err := jsonStorage.GetWallet()
    if err != nil {
        return fmt.Errorf("failed to get JSON wallet: %v", err)
    }

    gormWallet, err := gormStorage.GetWallet()
    if err != nil {
        return fmt.Errorf("failed to get GORM wallet: %v", err)
    }

    // Compare critical fields
    checks := []struct {
        name string
        valid bool
    }{
        {"Mnemonic", jsonWallet.Mnemonic == gormWallet.Mnemonic},
        {"CurrentIndex", jsonWallet.CurrentIndex == gormWallet.CurrentIndex},
        {"BackedUp", jsonWallet.BackedUp == gormWallet.BackedUp},
        {"AddressCount", len(jsonWallet.Addresses) == len(gormWallet.Addresses)},
    }

    allValid := true
    for _, check := range checks {
        if check.valid {
            log.Printf("   ✅ %s matches", check.name)
        } else {
            log.Printf("   ❌ %s MISMATCH!", check.name)
            allValid = false
        }
    }

    // Compare addresses
    for i, jsonAddr := range jsonWallet.Addresses {
        if i >= len(gormWallet.Addresses) {
            log.Printf("   ❌ Address %d missing in GORM", i)
            allValid = false
            continue
        }

        gormAddr := gormWallet.Addresses[i]
        if jsonAddr.Address != gormAddr.Address {
            log.Printf("   ❌ Address %d mismatch: %s != %s", i, jsonAddr.Address, gormAddr.Address)
            allValid = false
        } else {
            log.Printf("   ✅ Address %d: %s", i, jsonAddr.Address[:20]+"...")
        }
    }

    if allValid {
        log.Println("✅ Migration verification PASSED")
        return nil
    } else {
        return fmt.Errorf("migration verification FAILED - data mismatch detected")
    }
}
```

---

### **Step 0.4: Safe Migration Workflow**

**New File:** `go-wallet/safe_migrate.go` (Standalone tool)
```go
package main

import (
    "flag"
    "log"
)

func main() {
    var (
        dryRun = flag.Bool("dry-run", true, "Dry run (don't modify anything)")
        force  = flag.Bool("force", false, "Force migration (skip safety checks)")
    )
    flag.Parse()

    log.Println("🔄 Wallet Migration Tool")
    log.Println("========================")

    if *dryRun {
        log.Println("🧪 DRY RUN MODE - No changes will be made")
    }

    // Step 1: Backup
    log.Println("\n📋 Step 1: Backing up current wallet...")
    if err := BackupWallet(); err != nil {
        log.Fatalf("❌ Backup failed: %v", err)
    }

    // Step 2: Load JSON wallet
    log.Println("\n📋 Step 2: Loading JSON wallet...")
    jsonStorage, err := NewJSONStorage(GetWalletPath())
    if err != nil {
        log.Fatalf("❌ Failed to load JSON: %v", err)
    }

    jsonWallet, err := jsonStorage.GetWallet()
    if err != nil {
        log.Fatalf("❌ Failed to get wallet: %v", err)
    }

    log.Printf("   ✅ Loaded wallet with %d addresses", len(jsonWallet.Addresses))
    log.Printf("   ✅ Current index: %d", jsonWallet.CurrentIndex)
    log.Printf("   ✅ Backed up: %v", jsonWallet.BackedUp)

    if *dryRun {
        log.Println("\n✅ DRY RUN COMPLETE - Wallet is readable")
        log.Println("   To perform actual migration, run:")
        log.Println("   go run safe_migrate.go -dry-run=false")
        return
    }

    // Step 3: Initialize GORM database
    log.Println("\n📋 Step 3: Initializing SQLite database...")
    gormStorage, err := NewGORMStorage("./wallet.db")
    if err != nil {
        log.Fatalf("❌ Failed to initialize GORM: %v", err)
    }
    defer gormStorage.Close()

    // Step 4: Migrate data
    log.Println("\n📋 Step 4: Migrating data to SQLite...")
    if err := MigrateJSONToGORM(jsonStorage, gormStorage); err != nil {
        log.Fatalf("❌ Migration failed: %v", err)
    }

    // Step 5: Verify migration
    log.Println("\n📋 Step 5: Verifying migration...")
    if err := VerifyMigration(jsonStorage, gormStorage); err != nil {
        log.Fatalf("❌ Verification failed: %v", err)
    }

    log.Println("\n🎉 MIGRATION COMPLETE!")
    log.Println("   ✅ Data migrated successfully")
    log.Println("   ✅ Verification passed")
    log.Println("   ✅ Original wallet.json backed up")
    log.Println("\n📝 Next steps:")
    log.Println("   1. Test with: STORAGE_MODE=gorm go run main.go")
    log.Println("   2. If successful, set STORAGE_MODE=gorm as default")
    log.Println("   3. Keep wallet.json as backup (don't delete!)")
}
```

**Usage:**
```bash
# Step 1: Dry run (test without changing anything)
go run safe_migrate.go -dry-run=true

# Step 2: If dry run succeeds, do real migration
go run safe_migrate.go -dry-run=false

# Step 3: Test new database
STORAGE_MODE=gorm go run main.go

# Step 4: If tests pass, keep using GORM
# Step 5: Keep wallet.json as backup (don't delete!)
```

---

## 🧪 **COMPLETE TESTING WORKFLOW**

### **Testing Order (Protects Your Real Wallet):**

```
1. Test Mode (Empty wallet)     ← Test wallet creation
    ↓
2. Dry Run (Read-only)          ← Verify real wallet readable
    ↓
3. Migration (Create backup)    ← Migrate to SQLite
    ↓
4. Verification (Compare data)  ← Ensure no data loss
    ↓
5. Test Mode GORM (Empty)       ← Test GORM wallet operations
    ↓
6. Production GORM              ← Use real wallet with SQLite
```

### **Phase 1: Test Wallet Creation (Empty Wallet)**

**Purpose:** Test go-wallet-toolbox wallet creation WITHOUT touching your real wallet

**Command:**
```bash
# Create empty test wallet
WALLET_TEST_MODE=true STORAGE_MODE=gorm go run main.go
```

**What Happens:**
1. ✅ Creates `./test_wallet/wallet.db` (NEW empty wallet)
2. ✅ Your real wallet (`%APPDATA%/BabbageBrowser/wallet.json`) **UNTOUCHED**
3. ✅ Can test wallet creation, address generation, etc.
4. ✅ Can delete `./test_wallet/` when done

**Test Checklist:**
- [ ] Wallet database created (`test_wallet/wallet.db`)
- [ ] Can generate addresses
- [ ] Can retrieve wallet info
- [ ] No errors in logs
- [ ] Real wallet still exists and unchanged

---

### **Phase 2: Migrate Your Real Wallet (With Backups)**

**Purpose:** Move your REAL wallet with coins to SQLite

#### **Step A: Backup (Automatic)**
```bash
# Wallet automatically backed up to:
%APPDATA%/BabbageBrowser/backups/2025-10-09_14-30-00/
    ├── wallet.json
    └── domainWhitelist.json
```

#### **Step B: Dry Run Migration (Read-Only Test)**
```bash
go run safe_migrate.go -dry-run=true
```

**Expected Output:**
```
🔄 Wallet Migration Tool
========================
🧪 DRY RUN MODE - No changes will be made

📋 Step 1: Backing up current wallet...
   ✅ Wallet backed up to: C:\Users\...\backups\2025-10-09_14-30-00

📋 Step 2: Loading JSON wallet...
   ✅ Loaded wallet with 9 addresses
   ✅ Current index: 9
   ✅ Backed up: true

✅ DRY RUN COMPLETE - Wallet is readable
   To perform actual migration, run:
   go run safe_migrate.go -dry-run=false
```

#### **Step C: Real Migration**
```bash
go run safe_migrate.go -dry-run=false
```

**Expected Output:**
```
📋 Step 3: Initializing SQLite database...
   ✅ Database created: wallet.db

📋 Step 4: Migrating data to SQLite...
   ✅ Migrated wallet metadata
   ✅ Migrated address 0: 1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv
   ✅ Migrated address 1: 1A7s8z...
   ... (all 9 addresses)

📋 Step 5: Verifying migration...
   ✅ Mnemonic matches
   ✅ CurrentIndex matches
   ✅ BackedUp matches
   ✅ AddressCount matches
   ✅ Address 0: 1MBdcYaWTB3dYByNV3dBoLxkz6ibgv6Hmv
   ... (verify all addresses)

🎉 MIGRATION COMPLETE!
```

#### **Step D: Test SQLite Wallet (With Your Real Data)**
```bash
STORAGE_MODE=gorm go run main.go
```

**Test Checklist:**
- [ ] Daemon starts successfully
- [ ] All 9 addresses load correctly
- [ ] Balance shows correct total
- [ ] Can generate new address (index 10)
- [ ] Can send transaction
- [ ] Domain whitelist works
- [ ] CEF browser connects
- [ ] Frontend displays wallet correctly

#### **Step E: Parallel Operation (Safety Net)**
```bash
# Use SQLite
STORAGE_MODE=gorm go run main.go

# Rollback to JSON if issues
STORAGE_MODE=json go run main.go

# Original wallet.json is NEVER deleted!
```

---

### **Phase 3: Domain Whitelist Migration**

**Should migrate?** **YES** - Better performance and statistics

**When to migrate?** After wallet migration succeeds

**Migration Command:**
```bash
# Included in safe_migrate.go
go run safe_migrate.go -dry-run=false
# Migrates both wallet AND whitelist
```

**Single Database for Everything:**
```
wallet.db (SQLite file)
├── wallets table         (mnemonic, currentIndex, backedUp)
├── addresses table       (address, publicKey, index)
├── whitelisted_domains   (domain, addedAt, isPermanent)
└── transactions table    (future: transaction history)
```

**Benefits:**
- ✅ Single file to backup
- ✅ Atomic operations (wallet + whitelist together)
- ✅ Fast domain lookups
- ✅ Usage statistics (which domains used most)

---

### **Safety Guarantees:**

| Safety Feature | Implementation |
|----------------|----------------|
| **Automatic Backups** | Before every migration |
| **Test Mode** | Separate test wallet directory |
| **Dry Run** | Test migration without changes |
| **Verification** | Compare JSON vs SQLite data |
| **Rollback** | Switch back to JSON anytime |
| **Original Preserved** | Never delete wallet.json |

### **Data Loss Prevention:**

```bash
# Original files (NEVER DELETED):
%APPDATA%/BabbageBrowser/wallet.json
%APPDATA%/BabbageBrowser/domainWhitelist.json

# Backups (Created automatically):
%APPDATA%/BabbageBrowser/backups/2025-10-09_14-30-00/wallet.json
%APPDATA%/BabbageBrowser/backups/2025-10-09_14-30-00/domainWhitelist.json

# New database:
%APPDATA%/BabbageBrowser/wallet.db

# All three exist simultaneously - zero data loss risk!
```

---

### **Step 0.5: Domain Whitelist Migration to SQLite**

**Should You Migrate Domain Whitelist to SQLite?**

**YES! Here's why:**

| Aspect | JSON File | SQLite Database |
|--------|-----------|-----------------|
| **Queries** | Load entire file | Query specific domains |
| **Concurrent Access** | Risk of corruption | Thread-safe |
| **Performance** | Reload file each check | Fast indexed lookups |
| **Data Integrity** | Manual validation | Database constraints |
| **Backup** | Separate file | Same database as wallet |

#### **Whitelist Storage Structure**

**Current JSON (`domainWhitelist.json`):**
```json
{
  "domains": [
    {
      "domain": "toolbsv.com",
      "addedAt": "2025-10-09T14:00:00Z",
      "isPermanent": true
    }
  ]
}
```

**New GORM Table:**
```go
type WhitelistedDomain struct {
    ID          uint      `gorm:"primaryKey"`
    Domain      string    `gorm:"uniqueIndex;not null"`
    AddedAt     time.Time `gorm:"not null"`
    IsPermanent bool      `gorm:"default:false"`
    LastUsed    time.Time
    RequestCount int      `gorm:"default:0"`
    CreatedAt   time.Time
    UpdatedAt   time.Time
}
```

**Benefits of SQL for Whitelist:**
- ✅ Fast domain lookups (indexed)
- ✅ Usage statistics (request count, last used)
- ✅ No file locks (thread-safe)
- ✅ Can query by date/usage
- ✅ Automatic timestamps

#### **Migration Code**

**File:** `go-wallet/storage_gorm.go` (add to existing file)
```go
// MigrateWhitelist migrates domain whitelist from JSON to GORM
func (gs *GORMStorage) MigrateWhitelist(jsonPath string) error {
    log.Println("🔄 Migrating domain whitelist to SQLite...")

    // Read JSON whitelist
    data, err := os.ReadFile(jsonPath)
    if err != nil {
        return fmt.Errorf("failed to read whitelist: %v", err)
    }

    var jsonWhitelist struct {
        Domains []struct {
            Domain      string    `json:"domain"`
            AddedAt     time.Time `json:"addedAt"`
            IsPermanent bool      `json:"isPermanent"`
        } `json:"domains"`
    }

    if err := json.Unmarshal(data, &jsonWhitelist); err != nil {
        return fmt.Errorf("failed to parse whitelist: %v", err)
    }

    // Migrate each domain
    for _, d := range jsonWhitelist.Domains {
        domain := &WhitelistedDomain{
            Domain:      d.Domain,
            AddedAt:     d.AddedAt,
            IsPermanent: d.IsPermanent,
            RequestCount: 0,
        }

        // Insert into database
        if err := gs.db.Create(domain).Error; err != nil {
            log.Printf("⚠️  Failed to migrate domain %s: %v", d.Domain, err)
            continue
        }

        log.Printf("   ✅ Migrated: %s", d.Domain)
    }

    log.Printf("✅ Whitelist migration complete: %d domains", len(jsonWhitelist.Domains))
    return nil
}

// IsDomainWhitelisted checks if domain is whitelisted (GORM version)
func (gs *GORMStorage) IsDomainWhitelisted(domain string) (bool, error) {
    var count int64
    err := gs.db.Model(&WhitelistedDomain{}).Where("domain = ?", domain).Count(&count).Error
    if err != nil {
        return false, err
    }

    // Update usage stats
    if count > 0 {
        gs.db.Model(&WhitelistedDomain{}).
            Where("domain = ?", domain).
            Updates(map[string]interface{}{
                "last_used": time.Now(),
                "request_count": gorm.Expr("request_count + 1"),
            })
    }

    return count > 0, nil
}

// AddDomainToWhitelist adds domain (GORM version)
func (gs *GORMStorage) AddDomainToWhitelist(domain string, isPermanent bool) error {
    whitelist := &WhitelistedDomain{
        Domain:      domain,
        AddedAt:     time.Now(),
        IsPermanent: isPermanent,
    }

    return gs.db.Create(whitelist).Error
}
```

#### **Updated Domain Whitelist Endpoints**

**File:** `go-wallet/main.go` (modify existing handlers)
```go
// /domain/whitelist/check - Updated for GORM
http.HandleFunc("/domain/whitelist/check", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)
    domain := r.URL.Query().Get("domain")

    // Use storage interface (works with JSON or GORM)
    whitelisted, err := storage.IsDomainWhitelisted(domain)
    if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }

    response := map[string]bool{"whitelisted": whitelisted}
    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(response)
})

// /domain/whitelist/add - Updated for GORM
http.HandleFunc("/domain/whitelist/add", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    var req struct {
        Domain      string `json:"domain"`
        IsPermanent bool   `json:"isPermanent"`
    }
    json.NewDecoder(r.Body).Decode(&req)

    // Use storage interface (works with JSON or GORM)
    err := storage.AddDomainToWhitelist(req.Domain, req.IsPermanent)
    if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }

    response := map[string]bool{"success": true}
    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(response)
})
```

---

### **Step 1.1: Add GORM Dependency**

**File:** `go-wallet/go.mod`
```go
require (
    github.com/bsv-blockchain/go-sdk v1.2.9
    github.com/bsv-blockchain/go-wallet-toolbox v0.x.x  // ADD
    gorm.io/driver/sqlite v1.6.0                        // ADD
    gorm.io/gorm v1.31.0                                // ADD
    // ... existing dependencies
)
```

**Commands:**
```bash
cd go-wallet
go get github.com/bsv-blockchain/go-wallet-toolbox@latest
go get gorm.io/driver/sqlite
go get gorm.io/gorm
go mod tidy
```

### **Step 1.2: Create Storage Layer Abstraction**

**New File:** `go-wallet/storage_interface.go`
```go
package main

import (
    "github.com/bsv-blockchain/go-wallet-toolbox/pkg/storage"
)

// StorageInterface abstracts storage operations
// Allows switching between JSON and GORM seamlessly
type StorageInterface interface {
    // Wallet operations
    GetWallet() (*Wallet, error)
    SaveWallet(*Wallet) error

    // Address operations
    GetAddresses() ([]AddressInfo, error)
    AddAddress(*AddressInfo) error

    // Transaction operations
    GetTransactions() ([]Transaction, error)
    AddTransaction(*Transaction) error

    // Close storage connection
    Close() error
}

// JSONStorage - existing implementation
type JSONStorage struct {
    walletPath string
}

// GORMStorage - new implementation using wallet-toolbox
type GORMStorage struct {
    db *storage.Storage
}
```

### **Step 1.3: Implement JSON Storage Adapter (Keep Current Code)**

**New File:** `go-wallet/storage_json.go`
```go
package main

// JSONStorage wraps existing JSON file operations
type JSONStorage struct {
    walletManager *WalletManager
}

func NewJSONStorage(walletPath string) (*JSONStorage, error) {
    // Use existing WalletManager code
    wm := &WalletManager{
        wallet: &Wallet{},
        logger: logrus.New(),
    }

    err := wm.LoadFromFile(walletPath)
    if err != nil {
        return nil, err
    }

    return &JSONStorage{
        walletManager: wm,
    }, nil
}

func (js *JSONStorage) GetWallet() (*Wallet, error) {
    return js.walletManager.wallet, nil
}

func (js *JSONStorage) SaveWallet(w *Wallet) error {
    js.walletManager.wallet = w
    return js.walletManager.SaveToFile(GetWalletPath())
}

// ... implement other interface methods wrapping existing code
```

### **Step 1.4: Implement GORM Storage Adapter (New)**

**New File:** `go-wallet/storage_gorm.go`
```go
package main

import (
    "github.com/bsv-blockchain/go-wallet-toolbox/pkg/storage"
    "gorm.io/driver/sqlite"
    "gorm.io/gorm"
)

type GORMStorage struct {
    db *gorm.DB
    storage *storage.Storage
}

func NewGORMStorage(dbPath string) (*GORMStorage, error) {
    // Initialize GORM database
    db, err := gorm.Open(sqlite.Open(dbPath), &gorm.Config{})
    if err != nil {
        return nil, err
    }

    // Initialize wallet-toolbox storage
    store := storage.New(db)

    // Run migrations
    err = store.Migrate()
    if err != nil {
        return nil, err
    }

    return &GORMStorage{
        db: db,
        storage: store,
    }, nil
}

func (gs *GORMStorage) GetWallet() (*Wallet, error) {
    // Query GORM for wallet data
    // Map to our Wallet struct
    // Return wallet
}

func (gs *GORMStorage) SaveWallet(w *Wallet) error {
    // Map our Wallet struct to GORM models
    // Save to database
}

// ... implement other interface methods
```

### **Step 1.5: Add Storage Mode Configuration**

**File:** `go-wallet/main.go` (top of file)
```go
const (
    StorageModeJSON = "json"
    StorageModeGORM = "gorm"
)

var (
    storageMode = StorageModeJSON  // Default to JSON (no breaking changes)
    storage StorageInterface
)

func initStorage() error {
    switch storageMode {
    case StorageModeJSON:
        s, err := NewJSONStorage(GetWalletPath())
        if err != nil {
            return err
        }
        storage = s
        log.Println("💾 Using JSON storage")

    case StorageModeGORM:
        s, err := NewGORMStorage("./wallet.db")
        if err != nil {
            return err
        }
        storage = s
        log.Println("🗄️  Using GORM storage (SQLite)")

    default:
        return fmt.Errorf("unknown storage mode: %s", storageMode)
    }

    return nil
}

func main() {
    // Read storage mode from environment variable
    if mode := os.Getenv("STORAGE_MODE"); mode != "" {
        storageMode = mode
    }

    // Initialize storage
    if err := initStorage(); err != nil {
        log.Fatalf("Failed to initialize storage: %v", err)
    }
    defer storage.Close()

    // Rest of main() unchanged...
}
```

### **Step 1.6: Data Migration Tool**

**New File:** `go-wallet/migrate_storage.go`
```go
package main

import "fmt"

// MigrateJSONToGORM migrates data from JSON files to GORM database
func MigrateJSONToGORM(jsonPath, dbPath string) error {
    log.Println("🔄 Starting JSON → GORM migration...")

    // 1. Load JSON data
    jsonStorage, err := NewJSONStorage(jsonPath)
    if err != nil {
        return fmt.Errorf("failed to load JSON: %v", err)
    }

    // 2. Initialize GORM database
    gormStorage, err := NewGORMStorage(dbPath)
    if err != nil {
        return fmt.Errorf("failed to initialize GORM: %v", err)
    }
    defer gormStorage.Close()

    // 3. Get wallet data from JSON
    wallet, err := jsonStorage.GetWallet()
    if err != nil {
        return fmt.Errorf("failed to get wallet: %v", err)
    }

    // 4. Save to GORM
    err = gormStorage.SaveWallet(wallet)
    if err != nil {
        return fmt.Errorf("failed to save wallet: %v", err)
    }

    // 5. Verify migration
    verifyWallet, err := gormStorage.GetWallet()
    if err != nil {
        return fmt.Errorf("failed to verify: %v", err)
    }

    // 6. Compare data
    if wallet.Mnemonic != verifyWallet.Mnemonic {
        return fmt.Errorf("migration verification failed: mnemonic mismatch")
    }

    log.Println("✅ Migration successful!")
    log.Printf("   - Migrated %d addresses", len(wallet.Addresses))
    log.Printf("   - Current index: %d", wallet.CurrentIndex)

    return nil
}
```

**Command to run migration:**
```bash
# Migrate data
STORAGE_MODE=gorm go run migrate_storage.go

# Test with JSON (existing behavior)
STORAGE_MODE=json go run main.go

# Test with GORM (new behavior)
STORAGE_MODE=gorm go run main.go
```

### **Step 1.7: Testing Strategy for Storage Migration**

**Tests to Run:**
1. ✅ Start with JSON, verify all data loads correctly
2. ✅ Run migration tool
3. ✅ Start with GORM, verify all data matches
4. ✅ Create new address (GORM mode)
5. ✅ Send transaction (GORM mode)
6. ✅ Check balance (GORM mode)
7. ✅ Compare outputs with JSON mode

**Rollback Plan:**
If GORM fails, set `STORAGE_MODE=json` and restart. No data lost.

---

## 🔄 PHASE 2: Wallet Integration (With go-wallet-toolbox)

### **Goal:** Replace custom wallet logic with go-wallet-toolbox while keeping HTTP API unchanged

### **Why This Approach?**
- ✅ **CEF bridge unchanged** - no C++ changes needed
- ✅ **Frontend unchanged** - no React changes needed
- ✅ **Transparent migration** - endpoints work the same way

### **Architecture:**
```
HTTP Endpoint (unchanged)
    ↓
Handler Function (modified - delegates to toolbox)
    ↓
go-wallet-toolbox Wallet (new)
    ↓
GORM Storage (from Phase 1)
```

### **Step 2.1: Initialize go-wallet-toolbox Wallet**

**File:** `go-wallet/main.go` (add after storage initialization)
```go
import (
    sdk "github.com/bsv-blockchain/go-sdk/wallet"
    "github.com/bsv-blockchain/go-wallet-toolbox/pkg/wallet"
    "github.com/bsv-blockchain/go-wallet-toolbox/pkg/services"
)

var (
    bsvWallet *wallet.Wallet  // NEW: go-wallet-toolbox wallet instance
)

func initWallet() error {
    // Initialize services (blockchain APIs)
    walletServices := services.NewWalletServices(
        services.WithARC(arcConfig),        // Transaction broadcast
        services.WithWOC(wocConfig),        // WhatsOnChain
        services.WithBitails(bitailsConfig), // Bitails API
    )

    // Create BRC-100 compliant wallet
    w, err := wallet.New(
        storage.(*GORMStorage).storage,  // Use GORM storage from Phase 1
        wallet.WithServices(walletServices),
        wallet.WithLogger(logger),
    )
    if err != nil {
        return fmt.Errorf("failed to create wallet: %v", err)
    }

    bsvWallet = w
    log.Println("✅ BRC-100 wallet initialized")

    return nil
}

func main() {
    // Initialize storage (Phase 1)
    if err := initStorage(); err != nil {
        log.Fatalf("Storage init failed: %v", err)
    }

    // Initialize wallet (Phase 2)
    if err := initWallet(); err != nil {
        log.Fatalf("Wallet init failed: %v", err)
    }

    // Rest of main() unchanged...
}
```

### **Step 2.2: Endpoint Migration Strategy**

**We'll migrate endpoints in 4 categories:**

1. **Category A: Standard BRC-100** - Direct mapping to toolbox
2. **Category B: Custom Wallet** - Keep existing, add toolbox option
3. **Category C: Hybrid** - Use both (existing + toolbox)
4. **Category D: CEF-specific** - No changes

---

## 📊 ENDPOINT-BY-ENDPOINT MIGRATION GUIDE

### **Category A: Standard BRC-100 Endpoints (Direct Mapping)**

These endpoints map directly to `sdk.Interface` methods in go-wallet-toolbox.

#### **Endpoint 1: `/getVersion`**

**Current Implementation:**
```go
http.HandleFunc("/getVersion", func(w http.ResponseWriter, r *http.Request) {
    response := map[string]interface{}{
        "version": "BitcoinBrowserWallet v0.0.1",
        "capabilities": []string{"getVersion", "getPublicKey", ...},
    }
    json.NewEncoder(w).Encode(response)
})
```

**New Implementation (delegates to toolbox):**
```go
http.HandleFunc("/getVersion", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    // Parse request body (BRC-100 uses POST with originator)
    var req struct {
        Originator string `json:"originator"`
    }
    json.NewDecoder(r.Body).Decode(&req)

    // Call go-wallet-toolbox
    result, err := bsvWallet.GetVersion(r.Context(), nil, req.Originator)
    if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }

    // Return result (already BRC-100 compliant format)
    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(result)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ✅ Response format: BRC-100 compliant

---

#### **Endpoint 2: `/getPublicKey`**

**Current Implementation:**
```go
http.HandleFunc("/getPublicKey", func(w http.ResponseWriter, r *http.Request) {
    address, err := walletService.walletManager.GetCurrentAddress()
    response := map[string]interface{}{
        "publicKey": address.PublicKey,
        "address": address.Address,
    }
    json.NewEncoder(w).Encode(response)
})
```

**New Implementation:**
```go
http.HandleFunc("/getPublicKey", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    var req struct {
        Originator      string                 `json:"originator"`
        IdentityKey     bool                   `json:"identityKey"`
        Reason          map[string]interface{} `json:"reason"`
        CounterpartyKey string                 `json:"counterparty"`
        Privileged      bool                   `json:"privileged"`
    }
    json.NewDecoder(r.Body).Decode(&req)

    // Call go-wallet-toolbox
    result, err := bsvWallet.GetPublicKey(r.Context(), &sdk.GetPublicKeyArgs{
        IdentityKey: req.IdentityKey,
        Reason:      req.Reason,
        Counterparty: req.CounterpartyKey,
        Privileged:  req.Privileged,
    }, req.Originator)

    if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(result)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ⚠️ Response format: Now includes protocol, keyID, etc. (BRC-100 compliant)
- ✅ **Fixes:** Correct key derivation (secp256k1)

---

#### **Endpoint 3: `/isAuthenticated`**

**Current Implementation:**
```go
http.HandleFunc("/isAuthenticated", func(w http.ResponseWriter, r *http.Request) {
    // Custom implementation
    response := map[string]bool{"authenticated": true}
    json.NewEncoder(w).Encode(response)
})
```

**New Implementation:**
```go
http.HandleFunc("/isAuthenticated", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    var req struct {
        Originator string `json:"originator"`
    }
    json.NewDecoder(r.Body).Decode(&req)

    // Call go-wallet-toolbox
    result, err := bsvWallet.IsAuthenticated(r.Context(), nil, req.Originator)
    if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(result)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ✅ Response format: BRC-100 compliant

---

#### **Endpoint 4: `/createSignature`**

**Current Implementation:**
```go
http.HandleFunc("/createSignature", func(w http.ResponseWriter, r *http.Request) {
    // Custom ECDSA signing
    var req struct {
        Message string `json:"message"`
    }
    json.NewDecoder(r.Body).Decode(&req)

    // Sign with wallet private key
    signature := customSign(req.Message)

    response := map[string]string{"signature": signature}
    json.NewEncoder(w).Encode(response)
})
```

**New Implementation:**
```go
http.HandleFunc("/createSignature", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    var req struct {
        Data        string                 `json:"data"`
        Originator  string                 `json:"originator"`
        Reason      map[string]interface{} `json:"reason"`
        Counterparty string                `json:"counterparty"`
    }
    json.NewDecoder(r.Body).Decode(&req)

    // Call go-wallet-toolbox
    result, err := bsvWallet.CreateSignature(r.Context(), &sdk.CreateSignatureArgs{
        Data:        []byte(req.Data),
        Reason:      req.Reason,
        Counterparty: req.Counterparty,
    }, req.Originator)

    if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(result)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ✅ **Fixes:** Correct signature format (secp256k1)

---

#### **Endpoint 5: `/createAction`**

**Current Implementation:**
```go
http.HandleFunc("/createAction", func(w http.ResponseWriter, r *http.Request) {
    // Placeholder implementation
    response := map[string]interface{}{
        "success": true,
        "actionId": "placeholder",
    }
    json.NewEncoder(w).Encode(response)
})
```

**New Implementation:**
```go
http.HandleFunc("/createAction", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    var req struct {
        Description string                 `json:"description"`
        Inputs      map[string]interface{} `json:"inputs"`
        Outputs     []sdk.Output           `json:"outputs"`
        Originator  string                 `json:"originator"`
    }
    json.NewDecoder(r.Body).Decode(&req)

    // Call go-wallet-toolbox
    result, err := bsvWallet.CreateAction(r.Context(), &sdk.CreateActionArgs{
        Description: req.Description,
        Inputs:      req.Inputs,
        Outputs:     req.Outputs,
    }, req.Originator)

    if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(result)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ✅ **Adds:** Real BRC-100 action creation

---

### **Category B: Custom Wallet Endpoints (Hybrid Approach)**

These are YOUR custom endpoints. We'll keep them but optionally use toolbox for underlying operations.

#### **Endpoint: `/wallet/status`**

**Current Implementation:**
```go
http.HandleFunc("/wallet/status", func(w http.ResponseWriter, r *http.Request) {
    exists := walletService.walletManager != nil
    response := map[string]bool{"exists": exists}
    json.NewEncoder(w).Encode(response)
})
```

**New Implementation (KEEP EXISTING):**
```go
http.HandleFunc("/wallet/status", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    // Check if GORM storage has wallet data
    exists := storage != nil

    // Optional: Also check toolbox wallet
    if bsvWallet != nil {
        // Wallet is initialized and ready
        exists = true
    }

    response := map[string]bool{"exists": exists}
    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(response)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ✅ Backwards compatible

---

#### **Endpoint: `/wallet/address/generate`**

**Current Implementation:**
```go
http.HandleFunc("/wallet/address/generate", func(w http.ResponseWriter, r *http.Request) {
    address, err := walletService.walletManager.GetNextAddress()
    response := map[string]interface{}{
        "address": address.Address,
        "publicKey": address.PublicKey,
        "index": address.Index,
    }
    json.NewEncoder(w).Encode(response)
})
```

**New Implementation (HYBRID):**
```go
http.HandleFunc("/wallet/address/generate", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    // Option 1: Use existing custom logic (KEEP FOR NOW)
    wallet, err := storage.GetWallet()
    if err != nil {
        http.Error(w, err.Error(), http.StatusInternalServerError)
        return
    }

    // Generate next address (existing logic)
    nextIndex := wallet.CurrentIndex
    // ... existing address generation code

    // Option 2: Could use toolbox for key derivation
    // publicKey, err := bsvWallet.GetPublicKey(...)

    response := map[string]interface{}{
        "address": address.Address,
        "publicKey": address.PublicKey,
        "index": address.Index,
    }

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(response)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ⚠️ Could optionally use toolbox key derivation (future improvement)

---

#### **Endpoint: `/transaction/send`**

**Current Implementation:**
```go
http.HandleFunc("/transaction/send", func(w http.ResponseWriter, r *http.Request) {
    // Custom transaction creation + signing + broadcasting
    var req TransactionRequest
    json.NewDecoder(r.Body).Decode(&req)

    // Your existing transaction builder
    tx, err := transactionBuilder.CreateTransaction(...)
    signedTx, err := transactionBuilder.SignTransaction(...)
    result, err := broadcaster.Broadcast(...)

    json.NewEncoder(w).Encode(result)
})
```

**New Implementation (HYBRID):**
```go
http.HandleFunc("/transaction/send", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    var req TransactionRequest
    json.NewDecoder(r.Body).Decode(&req)

    // Option 1: Keep existing custom logic (working, don't break it)
    if useCustomTransactions {
        tx, err := transactionBuilder.CreateTransaction(...)
        signedTx, err := transactionBuilder.SignTransaction(...)
        result, err := broadcaster.Broadcast(...)

        w.Header().Set("Content-Type", "application/json")
        json.NewEncoder(w).Encode(result)
        return
    }

    // Option 2: Use toolbox (future migration)
    // createAction → signAction → processAction
    // This is the BRC-100 way

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(result)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ✅ **Keep existing working code**
- ⚠️ Can migrate to BRC-100 actions later

---

### **Category C: Authentication Endpoints (Critical Fixes)**

These are the endpoints causing nonce verification failures.

#### **Endpoint: `/.well-known/auth` (Babbage Authentication)**

**Current Implementation:**
```go
http.HandleFunc("/.well-known/auth", func(w http.ResponseWriter, r *http.Request) {
    // Custom BRC-42 signing (BROKEN - uses P-256)
    signature, err := signWithDerivedKey(dataToSign, privateKeyHex, invoiceNumber, authReq.IdentityKey)
    if err != nil {
        http.Error(w, "Failed to sign", http.StatusInternalServerError)
        return
    }

    // Return auth response
    authResponse := map[string]interface{}{
        "version": "0.1",
        "identityKey": currentAddress.PublicKey,
        "nonce": ourNonce,
        "yourNonce": theirNonce,
        "signature": hex.EncodeToString(signature),
    }
    json.NewEncoder(w).Encode(authResponse)
})
```

**New Implementation (USE TOOLBOX):**
```go
http.HandleFunc("/.well-known/auth", func(w http.ResponseWriter, r *http.Request) {
    enableCORS(w, r)

    var authReq struct {
        Version     string `json:"version"`
        MessageType string `json:"messageType"`
        IdentityKey string `json:"identityKey"`
        InitialNonce string `json:"initialNonce"`
    }

    if err := json.NewDecoder(r.Body).Decode(&authReq); err != nil {
        http.Error(w, "Bad request", http.StatusBadRequest)
        return
    }

    // Generate our nonce
    ourNonce := make([]byte, 32)
    rand.Read(ourNonce)
    ourNonceBase64 := base64.StdEncoding.EncodeToString(ourNonce)

    // Concatenate nonces for signing
    dataToSign := authReq.InitialNonce + ourNonceBase64

    // Use toolbox to create signature (CORRECT secp256k1 signing)
    sigResult, err := bsvWallet.CreateSignature(r.Context(), &sdk.CreateSignatureArgs{
        Data:        []byte(dataToSign),
        Reason:      map[string]interface{}{"type": "babbage-auth"},
        Counterparty: authReq.IdentityKey,
        // BRC-43 protocol ID
        ProtocolID:   [2]interface{}{2, "auth message signature"},
        KeyID:        authReq.InitialNonce + " " + ourNonceBase64,
    }, "babbage-auth")

    if err != nil {
        log.Printf("Signature creation failed: %v", err)
        http.Error(w, "Failed to sign", http.StatusInternalServerError)
        return
    }

    // Get our identity key
    pubKeyResult, err := bsvWallet.GetPublicKey(r.Context(), &sdk.GetPublicKeyArgs{
        IdentityKey: true,
    }, "babbage-auth")

    if err != nil {
        http.Error(w, "Failed to get public key", http.StatusInternalServerError)
        return
    }

    // Return BRC-104 compliant response
    authResponse := map[string]interface{}{
        "version":     "0.1",
        "messageType": "initialResponse",
        "identityKey": pubKeyResult.PublicKey,
        "nonce":       ourNonceBase64,
        "yourNonce":   authReq.InitialNonce,
        "signature":   sigResult.Signature, // CORRECT secp256k1 signature!
    }

    w.Header().Set("Content-Type", "application/json")
    json.NewEncoder(w).Encode(authResponse)
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ✅ **FIXES:** Correct secp256k1 signature
- ✅ **FIXES:** Proper BRC-42/43 key derivation
- ✅ **SHOULD FIX:** Nonce verification failures on Babbage sites

---

### **Category D: CEF-Specific Endpoints (No Changes)**

#### **Endpoint: `/domain/whitelist/add`**

**Implementation:** KEEP AS-IS

```go
http.HandleFunc("/domain/whitelist/add", func(w http.ResponseWriter, r *http.Request) {
    // This is CEF-specific functionality
    // Not part of BRC-100
    // Keep existing implementation unchanged
    // ...
})
```

**Impact:**
- ✅ CEF: No changes
- ✅ Frontend: No changes
- ✅ Keep all domain whitelist functionality

---

## 📋 COMPLETE ENDPOINT MIGRATION CHECKLIST

### **Standard BRC-100 Endpoints (Use Toolbox)**

| Endpoint | Current Status | Migration | Impact | Priority |
|----------|---------------|-----------|--------|----------|
| `/getVersion` | Placeholder | ✅ Direct map to toolbox | None | High |
| `/getPublicKey` | Working | ✅ Use toolbox (fixes curve) | Better | High |
| `/isAuthenticated` | Placeholder | ✅ Direct map to toolbox | None | High |
| `/createSignature` | Custom | ✅ Use toolbox (fixes curve) | Better | High |
| `/createAction` | Placeholder | ✅ Implement with toolbox | New feature | Medium |
| `/signAction` | Placeholder | ✅ Implement with toolbox | New feature | Medium |
| `/abortAction` | Placeholder | ✅ Implement with toolbox | New feature | Low |
| `/listActions` | Placeholder | ✅ Implement with toolbox | New feature | Low |
| `/internalizeAction` | Missing | ✅ Implement with toolbox | New feature | Low |
| `/listOutputs` | Missing | ✅ Implement with toolbox | New feature | Low |
| `/relinquishOutput` | Missing | ✅ Implement with toolbox | New feature | Low |
| `/encrypt` | Missing | ✅ Implement with toolbox | New feature | Low |
| `/decrypt` | Missing | ✅ Implement with toolbox | New feature | Low |
| `/createHmac` | Missing | ✅ Implement with toolbox | New feature | Low |
| `/verifyHmac` | Missing | ✅ Implement with toolbox | New feature | Low |

### **Custom Wallet Endpoints (Keep Existing)**

| Endpoint | Current Status | Migration | Impact | Priority |
|----------|---------------|-----------|--------|----------|
| `/wallet/status` | Working | ⚠️ Keep, adapt check | Minimal | Low |
| `/wallet/balance` | Working | ✅ Keep as-is | None | Low |
| `/wallet/addresses` | Working | ✅ Keep as-is | None | Low |
| `/wallet/address/generate` | Working | ✅ Keep as-is | None | Low |
| `/transaction/send` | Working | ✅ Keep as-is | None | **NONE** |
| `/transaction/create` | Working | ✅ Keep as-is | None | Low |
| `/transaction/sign` | Working | ✅ Keep as-is | None | Low |
| `/transaction/broadcast` | Working | ✅ Keep as-is | None | Low |
| `/utxo/fetch` | Working | ✅ Keep as-is | None | Low |

### **Authentication Endpoints (Critical Fixes)**

| Endpoint | Current Status | Migration | Impact | Priority |
|----------|---------------|-----------|--------|----------|
| `/.well-known/auth` | Broken | ✅ **FIX with toolbox** | **Fixes auth** | **CRITICAL** |
| `/socket.io/` | Partial | ⚠️ Review with toolbox | May improve | Medium |

### **Domain/CEF Endpoints (No Changes)**

| Endpoint | Current Status | Migration | Impact | Priority |
|----------|---------------|-----------|--------|----------|
| `/domain/whitelist/*` | Working | ✅ Keep as-is | None | Low |

---

## 🧪 PHASE 3: Testing Strategy

### **Test Plan Overview:**

1. **Unit Tests** - Test each migrated endpoint
2. **Integration Tests** - Test complete workflows
3. **Compatibility Tests** - Test with real sites
4. **Performance Tests** - Compare old vs new
5. **Security Tests** - Verify key derivation

### **Testing Checklist:**

#### **Unit Tests (Per Endpoint)**
- [ ] `/getVersion` returns correct format
- [ ] `/getPublicKey` returns secp256k1 public key
- [ ] `/isAuthenticated` works for different originators
- [ ] `/createSignature` produces valid signatures
- [ ] `/createAction` creates BRC-100 actions
- [ ] `/.well-known/auth` signs nonces correctly

#### **Integration Tests**
- [ ] Complete transaction flow (create → sign → broadcast)
- [ ] Address generation and balance checking
- [ ] Authentication flow (nonce exchange)
- [ ] BEEF transaction creation and verification

#### **Compatibility Tests (Real Sites)**

**ToolBSV.com:**
- [ ] Wallet detection works
- [ ] `/getVersion` returns proper response
- [ ] `/getPublicKey` returns correct format
- [ ] Site recognizes wallet as BRC-100 compliant

**Babbage Sites (peerpay, thryll, etc.):**
- [ ] `/.well-known/auth` authentication succeeds
- [ ] Nonce verification passes
- [ ] No "Initial response nonce verification failed" errors
- [ ] Socket.IO connection works

**Your Browser:**
- [ ] Wallet panel opens and displays balance
- [ ] Send transaction works
- [ ] Receive address generation works
- [ ] Settings panel works
- [ ] Domain whitelist works

#### **Performance Tests**
- [ ] Startup time (JSON vs GORM)
- [ ] Transaction creation speed
- [ ] Signature generation speed
- [ ] Database query performance

#### **Security Tests**
- [ ] Private keys never exposed in responses
- [ ] Signatures use secp256k1 (not P-256)
- [ ] BRC-42 key derivation correct
- [ ] Domain whitelist still enforced

---

## 🔄 PHASE 4: Cutover & Cleanup

### **Cutover Plan:**

1. **Final Testing** (2-3 days)
   - All tests passing
   - No regressions
   - Performance acceptable

2. **Production Switch** (1 hour)
   ```bash
   # Set environment variable
   export STORAGE_MODE=gorm

   # Restart daemon
   ./bitcoin-wallet.exe
   ```

3. **Monitor** (1 week)
   - Check logs for errors
   - Monitor Babbage sites
   - Check ToolBSV compatibility

4. **Cleanup** (2-3 days)
   - Remove old JSON storage code
   - Remove custom BRC-42 implementation
   - Update documentation

### **Rollback Plan:**

If issues arise:
```bash
# Switch back to JSON
export STORAGE_MODE=json

# Restart daemon
./bitcoin-wallet.exe

# Data is safe - GORM doesn't delete JSON files
```

---

## 🎯 SUCCESS CRITERIA

### **Must Have (Phase 1 & 2):**
- ✅ All endpoints respond correctly
- ✅ ToolBSV recognizes wallet
- ✅ Babbage auth succeeds (no nonce errors)
- ✅ Transactions still work
- ✅ No frontend changes needed

### **Nice to Have (Phase 3 & 4):**
- ✅ All BRC-100 endpoints implemented
- ✅ GORM storage production-ready
- ✅ Performance improvements
- ✅ Clean codebase

---

## 📊 RISK ASSESSMENT

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Storage migration breaks wallet | Low | High | Keep JSON as backup |
| Toolbox incompatible with CEF | Low | Medium | Use HTTP wrapper layer |
| Frontend requires changes | Very Low | High | Design prevents this |
| Performance degradation | Low | Medium | Test and optimize |
| New bugs introduced | Medium | Medium | Comprehensive testing |

---

## 📚 RESOURCES & REFERENCES

### **Documentation to Read:**
1. `reference/go-wallet-toolbox/README.md` - Main documentation
2. `reference/go-wallet-toolbox/examples/README.md` - Usage examples
3. `reference/go-wallet-toolbox/docs/wallet.md` - Wallet API
4. `reference/go-wallet-toolbox/docs/storage.md` - Storage layer

### **Code to Study:**
1. `reference/go-wallet-toolbox/pkg/wallet/wallet.go` - Main wallet
2. `reference/go-wallet-toolbox/pkg/storage/` - Storage implementation
3. `reference/go-wallet-toolbox/examples/wallet_examples/` - Example usage

### **Key Dependencies:**
```
github.com/bsv-blockchain/go-wallet-toolbox v0.x.x
github.com/bsv-blockchain/go-sdk v1.2.10
gorm.io/gorm v1.31.0
gorm.io/driver/sqlite v1.6.0
```

---

## 🎯 NEXT IMMEDIATE STEPS

1. **Review this plan** - Discuss and adjust
2. **Study examples** - Understand toolbox usage
3. **Start Phase 1** - Storage abstraction
4. **Test frequently** - Don't break existing functionality
5. **Document progress** - Update this file as you go

---

## 📝 NOTES & OBSERVATIONS

### **Key Insights:**
- go-wallet-toolbox is production-ready and well-documented
- Migration can be done incrementally without breaking changes
- Frontend and CEF remain completely unchanged
- Storage migration is independent from wallet migration
- Critical auth bugs will be fixed by correct secp256k1 usage

### **Decision Points:**
- SQLite vs Postgres (start with SQLite)
- Keep custom transaction endpoints (yes, they work)
- Migrate all BRC-100 endpoints at once (no, do incrementally)

### **Open Questions:**
- [ ] How does toolbox handle HD wallet import from mnemonic?
- [ ] Can we use toolbox services layer for existing UTXO fetching?
- [ ] Does toolbox support our multi-miner broadcasting?
- [ ] How to handle domain whitelist with toolbox?

---

## 🚀 **RECOMMENDED ACTION PLAN**

### **This Week: Research & Planning**

**Day 1-2: Study go-wallet-toolbox**
```bash
# Read documentation
cd reference/go-wallet-toolbox
cat README.md
cat docs/wallet.md
cat docs/storage.md

# Study examples
cd examples/wallet_examples
cat README.md

# Look at test wallet example
cd create_p2pkh_tx
cat create_p2pkh_tx.go
```

**Day 3-4: Detailed Planning**
- [ ] Map each of your 33 endpoints to toolbox methods
- [ ] Identify endpoints to keep vs replace
- [ ] Plan storage schema (wallet + whitelist tables)
- [ ] Design test strategy

**Day 5: Set up test environment**
```bash
# Create test mode infrastructure
# Implement backup_wallet.go
# Implement test_mode.go
# Test with empty wallet
```

---

### **Next Week: Implementation (Phase 1)**

**Phase 1.1: Storage Abstraction (2-3 days)**
- [ ] Create `storage_interface.go`
- [ ] Implement `storage_json.go` (wrap existing code)
- [ ] Implement `storage_gorm.go` (new SQLite)
- [ ] Test both modes work

**Phase 1.2: Migration Tools (1-2 days)**
- [ ] Implement `safe_migrate.go`
- [ ] Implement `verify_migration.go`
- [ ] Test dry run mode
- [ ] Test with empty test wallet

**Phase 1.3: Real Wallet Migration (1 day)**
- [ ] Backup your wallet (automatic)
- [ ] Run migration on real wallet
- [ ] Verify all data migrated
- [ ] Test wallet operations with SQLite

---

### **Week 3: Implementation (Phase 2)**

**Phase 2.1: Toolbox Integration (3-4 days)**
- [ ] Add go-wallet-toolbox dependency
- [ ] Initialize toolbox wallet
- [ ] Migrate authentication endpoints
- [ ] Test with ToolBSV

**Phase 2.2: Endpoint Migration (2-3 days)**
- [ ] Migrate standard BRC-100 endpoints
- [ ] Keep custom endpoints working
- [ ] Test all endpoints

---

### **Week 4: Testing & Validation**

**Testing (5-7 days)**
- [ ] Unit tests (each endpoint)
- [ ] Integration tests (complete flows)
- [ ] ToolBSV compatibility test
- [ ] Babbage sites test (peerpay, thryll)
- [ ] Your browser functionality test
- [ ] Performance testing

**Cleanup & Documentation (2-3 days)**
- [ ] Remove old code
- [ ] Update documentation
- [ ] Merge to main

---

## 📋 **IMMEDIATE NEXT STEPS (Today)**

### **Step 1: Review This Plan**
- [ ] Read through entire document
- [ ] Ask questions about unclear sections
- [ ] Identify concerns or risks

### **Step 2: Study go-wallet-toolbox**
```bash
cd reference/go-wallet-toolbox

# Read main docs
cat README.md
cat examples/README.md

# Look at wallet implementation
cat pkg/wallet/wallet.go

# Study examples
cd examples/wallet_examples/create_p2pkh_tx
cat create_p2pkh_tx.go
```

**Focus on:**
- How they create wallets
- How they handle key derivation
- How they implement BRC-100 methods
- Storage layer usage

### **Step 3: Create Test Environment Setup (Tomorrow)**

**Files to create:**
1. `go-wallet/backup_wallet.go` - Automatic backups
2. `go-wallet/test_mode.go` - Test wallet isolation
3. `go-wallet/storage_interface.go` - Storage abstraction

**Test:**
```bash
# Test with empty wallet (safe)
WALLET_TEST_MODE=true go run main.go
```

### **Step 4: Prototype Storage Migration (This Week)**

**Goal:** Get comfortable with GORM and SQLite

**Tasks:**
- [ ] Add GORM dependency
- [ ] Create simple GORM storage
- [ ] Test wallet creation in GORM
- [ ] Test address generation in GORM

---

## 🎯 **SUCCESS CRITERIA**

### **Phase 1 Complete When:**
- ✅ Test wallet works in GORM mode
- ✅ Real wallet migrated to SQLite
- ✅ All addresses and data verified
- ✅ Can switch between JSON/GORM seamlessly
- ✅ Domain whitelist in SQLite
- ✅ Zero data loss

### **Phase 2 Complete When:**
- ✅ ToolBSV detects wallet
- ✅ Authentication succeeds (no nonce errors)
- ✅ All standard BRC-100 endpoints work
- ✅ Custom endpoints still work
- ✅ CEF and frontend unchanged

### **Final Success Criteria:**
- ✅ ToolBSV fully functional
- ✅ Babbage sites authenticate successfully
- ✅ Your wallet/browser fully functional
- ✅ Production-ready storage
- ✅ BRC-100 compliant

---

**Last Updated:** October 10, 2025
**Status:** Planning Phase - Ready to Begin
**Branch:** `go-wallet-toolkit-migration`
**Next Action:** Study go-wallet-toolbox examples
