# PushDrop Testing Strategy

## Problem: Avoiding Circular Validation

We need to test PushDrop without creating self-validating tests that just confirm our (potentially incorrect) understanding.

## Testing Approaches (Ranked by Reliability)

### 1. ✅ **Cross-Implementation Validation** (BEST)
**Use TypeScript SDK to generate test vectors, validate with Rust**

- Generate scripts with TypeScript SDK (`@bsv/sdk`)
- Decode with Rust implementation
- Compare results
- **Advantage**: Uses reference implementation as source of truth
- **Disadvantage**: Requires Node.js setup

### 2. ✅ **Bitcoin Script Standard Validation** (GOOD)
**Test parser against well-defined Bitcoin script standards**

- Test opcode parsing (OP_0, OP_1-OP_16, OP_1NEGATE) - these are Bitcoin standard
- Test OP_PUSHDATA1/2/4 parsing - well-documented Bitcoin script format
- Test minimal encoding rules - these are optimization rules, not protocol
- **Advantage**: Tests against Bitcoin protocol standards, not our interpretation
- **Disadvantage**: Doesn't test PushDrop-specific logic

### 3. ✅ **Real Blockchain Data** (BEST - When Available)
**Extract real PushDrop scripts from blockchain certificate transactions**

- Find certificate transactions on blockchain
- Extract locking scripts from outputs
- Decode with Rust implementation
- Verify fields match expected certificate structure
- **Advantage**: Tests against real-world data
- **Disadvantage**: Requires finding real certificate transactions

### 4. ⚠️ **Round-Trip Testing** (LIMITED VALUE)
**Encode then decode - only validates internal consistency**

- Encode fields → script
- Decode script → fields
- Compare original vs decoded
- **Advantage**: Catches obvious bugs
- **Disadvantage**: Circular validation - doesn't prove correctness

### 5. ✅ **Component Isolation** (GOOD)
**Test individual components separately**

- Test parser independently (against Bitcoin standards)
- Test minimal encoding independently (against known patterns)
- Test field extraction independently (with known script structures)
- **Advantage**: Isolates bugs to specific components
- **Disadvantage**: Doesn't test integration

## Recommended Testing Plan

### Phase 1: Component Tests (Safe - No Circular Validation)
1. **Parser Tests**: Test against Bitcoin script standards
   - OP_0, OP_1-OP_16, OP_1NEGATE (well-defined)
   - OP_PUSHDATA1/2/4 (well-documented)
   - Direct pushes (opcode 1-75)

2. **Minimal Encoding Tests**: Test against known patterns
   - Empty → OP_0
   - [1-16] → OP_1-OP_16
   - [0x81] → OP_1NEGATE
   - Length boundaries (75, 255, 65535)

### Phase 2: Cross-Implementation Tests (Best Validation)
1. **TypeScript → Rust**: Generate scripts with TS, decode with Rust
2. **Rust → TypeScript**: Generate scripts with Rust, decode with TS
3. **Compare Results**: Fields and public keys should match

### Phase 3: Real-World Validation (Ultimate Test)
1. Find real certificate transactions on blockchain
2. Extract PushDrop scripts
3. Decode and verify structure matches expected certificate format

## Implementation

See `test_pushdrop_cross_validation.ps1` for cross-implementation tests using TypeScript SDK.
