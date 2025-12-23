# Hodos Browser Feature Research Documentation

## Overview

This directory contains comprehensive research and implementation documentation for four major browser features to be added to the Hodos Browser:

1. **History Management**
2. **Bookmarks System**
3. **Favorites (Speed Dial)**
4. **Cookies Management**

## Project Context

The Hodos Browser is built on:
- **CEF (Chromium Embedded Framework)**: Native runtime for browser functionality
- **Rust Backend**: HTTP API server (Actix-web) on port 3301 for wallet and browser operations
- **TypeScript Frontend**: React-based UI compiled with Vite
- **BSV Blockchain Integration**: BRC-100 wallet functionality built-in

## Documentation Structure

Each feature documentation file contains:

### Technical Specifications
- Database schema design (SQLite)
- Data models and structures
- API endpoint definitions

### Implementation Architecture
- Layer 1: Rust backend database repositories
- Layer 2: HTTP API endpoints (Actix-web)
- Layer 3: CEF native integration (C++)
- Layer 4: Frontend TypeScript/React implementation

### Code Examples
- Complete, production-ready code snippets
- Database queries and migrations
- API request/response formats
- React components and hooks

### Implementation Roadmap
- Phased development approach
- Step-by-step implementation guides
- Testing strategies
- Performance optimization techniques

## Feature Documentation Files

### 1. HISTORY_FEATURE.md

**Comprehensive browser history tracking and management**

Key Components:
- SQLite database schema matching Chromium's history format
- Visit tracking with timestamps and transition types
- Search functionality with pagination
- Integration with CEF's OnLoadEnd handler
- React UI for browsing and managing history

Database Tables:
- `urls` - Stores unique URLs with metadata
- `visits` - Individual visit records with timestamps
- `visit_source` - Visit origin tracking

API Endpoints:
- `/history/add` - Record new visit
- `/history/get` - Retrieve history with search/pagination
- `/history/delete` - Remove individual entries
- `/history/clear` - Clear all or date-range history

### 2. BOOKMARKS_FEATURE.md

**Full-featured bookmark management system**

Key Components:
- Hierarchical folder structure
- Tag-based organization
- Search and filtering
- Import/export functionality
- Visit count tracking

Database Tables:
- `bookmarks` - Main bookmark records with folder support
- `bookmark_tags` - Many-to-many tag relationships
- `bookmark_metadata` - Extensible metadata storage

API Endpoints:
- `/bookmarks/add` - Create bookmark or folder
- `/bookmarks/{id}` - Get/delete bookmark
- `/bookmarks/list` - List bookmarks in folder
- `/bookmarks/search` - Full-text search
- `/bookmarks/update` - Modify bookmark properties
- `/bookmarks/move` - Reorganize hierarchy
- `/bookmarks/tags` - Tag management
- `/bookmarks/export` - Export as JSON
- `/bookmarks/import` - Import bookmarks

### 3. FAVORITES_FEATURE.md

**Visual speed dial with smart suggestions**

Key Components:
- Grid-based visual layout
- Thumbnail/screenshot capture
- Auto-population from most visited
- Drag-and-drop reordering
- Integration with bookmark system

Implementation Approach:
- Extends bookmark system with `is_favorite` flag
- Thumbnail storage (Base64 encoded)
- Smart suggestion algorithm based on visit patterns
- New tab page integration

API Endpoints:
- `/favorites` - Get all favorites
- `/favorites/add` - Mark bookmark as favorite
- `/favorites/{id}` - Remove from favorites
- `/favorites/thumbnail` - Update thumbnail
- `/favorites/reorder` - Change display order
- `/favorites/suggestions` - Get smart suggestions
- `/favorites/auto-populate` - Auto-fill from history

### 4. COOKIES_MANAGEMENT_FEATURE.md

**Privacy-focused cookie management**

Key Components:
- Leverages CEF's built-in CefCookieManager
- User-facing cookie viewer and editor
- Domain-based preferences
- Privacy controls (block third-party, session-only)
- Cookie audit and analytics

Implementation Approach:
- CEF CookieVisitor pattern for enumeration
- Metadata storage for user preferences
- Domain grouping and search
- Secure/HttpOnly indicators

Database Tables:
- `cookie_metadata` - User annotations and flags
- `cookie_preferences` - Per-domain settings

API Endpoints:
- `/cookies/preferences` - Get/set domain preferences
- `/cookies/preferences/{domain}` - Delete preferences

CEF Integration:
- CefCookieManager for native cookie access
- Custom CookieVisitor for enumeration
- Delete and set operations through CEF API

## Common Implementation Patterns

### Database Layer (Rust)

All features follow this repository pattern:

```rust
pub struct FeatureRepository {
    db_path: String,
}

impl FeatureRepository {
    pub fn new(db_path: String) -> Self;
    pub fn initialize(&self) -> SqliteResult<()>;
    // CRUD operations
}
```

### HTTP API Layer (Actix-web)

Standard endpoint structure:

```rust
pub async fn endpoint_handler(
    data: web::Json<RequestType>,
    repo: web::Data<Repository>,
) -> ActixResult<HttpResponse> {
    match repo.operation() {
        Ok(result) => Ok(HttpResponse::Ok().json(json!({
            "success": true,
            "data": result
        }))),
        Err(e) => Ok(HttpResponse::InternalServerError().json(json!({
            "success": false,
            "error": e.to_string()
        })))
    }
}
```

### CEF Native Layer (C++)

Handler integration pattern:

```cpp
class FeatureHandler {
public:
    static FeatureHandler& GetInstance();
    void HandleOperation(params...);
private:
    void SendToBackend(data...);
};
```

### Frontend Layer (TypeScript/React)

Hook-based state management:

```typescript
export function useFeature() {
  const [data, setData] = useState<Type[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadData = useCallback(async () => {
    // API call through bridge
  }, []);

  return { data, loading, error, operations... };
}
```

## Development Workflow

### 1. Backend First
- Implement database repository
- Create and test database schema
- Build HTTP API endpoints
- Test with Postman/curl

### 2. CEF Integration
- Create native handlers
- Integrate with existing CEF infrastructure
- Implement message passing to/from JavaScript
- Test in browser context

### 3. Frontend Implementation
- Define TypeScript types
- Extend window.hodosBrowser API
- Create React hooks
- Build UI components
- Integrate with existing UI

### 4. Testing and Refinement
- Unit tests for each layer
- Integration tests end-to-end
- Performance optimization
- Security hardening

## Technology Stack Reference

### Backend
- **Language**: Rust (stable)
- **HTTP Framework**: Actix-web 4.11.0
- **Database**: SQLite (rusqlite with bundled)
- **Serialization**: Serde + serde_json
- **Async Runtime**: Tokio

### CEF Native
- **Language**: C++17
- **Framework**: CEF (Chromium Embedded Framework)
- **Build System**: CMake
- **JSON**: nlohmann/json
- **Compiler**: MSVC (Windows)

### Frontend
- **Language**: TypeScript
- **Framework**: React 19.1.0
- **Build Tool**: Vite 6.3.5
- **Router**: React Router 7.6.1
- **UI Library**: Material-UI 7.1.1
- **Styling**: Emotion

## Build and Deployment

### Backend Build
```bash
cd rust-wallet
cargo build --release
# Output: target/release/hodos-wallet.exe
```

### Frontend Build
```bash
cd frontend
npm run build
# Output: dist/
```

### CEF Native Build
```bash
cd cef-native
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build --config Release
# Output: build/bin/HodosBrowserShell.exe
```

## Integration Points

### Existing Systems

All features integrate with:
- **Wallet Backend**: Shared database connection pool
- **CEF Browser**: Shared message bus and handlers
- **React Frontend**: Shared component library and hooks
- **Settings System**: Configuration and preferences

### Message Flow

```
User Action (Frontend)
  ↓
window.hodosBrowser API (Bridge)
  ↓
window.cefMessage.send() (V8 → CEF)
  ↓
CEF Render Process Handler
  ↓
CEF Browser Process Handler
  ↓
HTTP Request to Rust Backend (port 3301)
  ↓
Actix-web Handler
  ↓
Database Repository
  ↓
SQLite Database
  ↓
Response back through chain
```

## Performance Considerations

### Database
- Proper indexing on all query columns
- Connection pooling
- Prepared statements for repeated queries
- Transaction batching for bulk operations

### API
- Pagination for large datasets (50-100 items)
- Async operations throughout
- Request/response caching
- Rate limiting where appropriate

### Frontend
- Virtual scrolling for long lists
- Lazy loading of components
- Debounced search inputs
- Optimistic UI updates
- React.memo for expensive components

## Security Guidelines

### Data Validation
- Validate all user inputs
- Sanitize URLs and text
- Use parameterized SQL queries
- Validate file uploads

### Privacy
- Respect incognito/private mode
- Implement secure deletion
- Handle sensitive data appropriately
- Optional encryption for sensitive storage

### Network
- HTTPS for external requests
- Certificate validation
- Secure cookie handling (HttpOnly, Secure flags)
- CORS configuration

## Testing Strategy

### Unit Tests
- Database operations (Rust)
- API handlers (Rust)
- Frontend hooks (TypeScript/Jest)
- Utility functions

### Integration Tests
- End-to-end feature workflows
- Cross-layer communication
- Database migrations
- API endpoint contracts

### UI Tests
- Component rendering
- User interaction flows
- Error states
- Loading states

## Next Steps

### Priority 1: History
- Most foundational feature
- Required for other features
- Simplest implementation

### Priority 2: Bookmarks
- Builds on database patterns
- Core browser functionality
- Enables Favorites

### Priority 3: Favorites
- Extends bookmarks
- Enhances UX
- New tab page integration

### Priority 4: Cookies
- Leverages CEF built-ins
- Privacy features
- Settings integration

## Resources

### CEF Documentation
- Official CEF API: https://bitbucket.org/chromiumembedded/cef
- CEF Forum: https://magpcss.org/ceforum/

### Rust Resources
- Actix-web Docs: https://actix.rs/
- Rusqlite: https://docs.rs/rusqlite/

### Frontend Resources
- React: https://react.dev/
- Material-UI: https://mui.com/
- TypeScript: https://www.typescriptlang.org/

## Contact and Support

For questions or clarifications:
- Review existing documentation in /docs
- Check Developer_notes.md for implementation details
- Refer to ARCHITECTURE.md for system overview
- Follow CLAUDE.md for development guidelines

## Conclusion

This documentation provides a complete blueprint for implementing browser history, bookmarks, favorites, and cookie management features in the Hodos Browser. Each feature is designed to integrate seamlessly with the existing architecture while following best practices for performance, security, and user experience.

The modular approach allows for independent development of each feature while maintaining consistency across the codebase. All code examples are production-ready and follow the established patterns in the Hodos Browser project.
