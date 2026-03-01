# Frontend Tests

## Directory Structure

```
frontend/
├── src/
│   └── components/
│       └── __tests__/           # Co-located component tests
│           └── WalletPanel.test.tsx
│
├── tests/                        # Integration tests & setup
│   ├── setup.ts                 # Vitest global setup
│   ├── mocks/                   # Shared mocks
│   │   └── api.ts              # Mock wallet API responses
│   └── README.md               # This file
│
└── vite.config.ts               # Vitest configuration
```

## Setup (not yet configured)

```bash
cd frontend
npm install -D vitest @testing-library/react @testing-library/jest-dom jsdom @vitest/coverage-v8
```

Add to `vite.config.ts`:
```typescript
export default defineConfig({
  // ... existing config
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './tests/setup.ts',
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html'],
    }
  }
});
```

Add to `package.json` scripts:
```json
{
  "scripts": {
    "test": "vitest",
    "test:coverage": "vitest --coverage"
  }
}
```

## Running Tests

```bash
# Interactive watch mode
npm test

# Single run (CI mode)
npm test -- --run

# With coverage
npm test -- --run --coverage
```

## Test Priority

| Priority | What to Test | Location |
|----------|--------------|----------|
| P1 | Utility functions (price formatting, validation) | `src/utils/__tests__/` |
| P2 | Hooks (useBalance, useBackgroundBalancePoller) | `src/hooks/__tests__/` |
| P2 | Form validation (DomainPermissionForm) | `src/components/__tests__/` |
| P3 | Component rendering | `src/components/__tests__/` |

## Mock Patterns

```typescript
// Mock wallet API
import { mockWalletApi } from '../tests/mocks/api';

beforeEach(() => {
  mockWalletApi({ balance: 100000, bsvPrice: 50.0 });
});
```
