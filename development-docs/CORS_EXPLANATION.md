# CORS (Cross-Origin Resource Sharing) - Brief Explanation

## What is CORS?

**CORS** is a browser security feature that **blocks cross-origin requests** unless the server explicitly allows them.

### Key Concepts:

1. **Same-Origin Policy**: Browsers block requests from one origin (domain) to another by default
   - **Origin** = Protocol + Domain + Port
   - Example: `https://microblog.bitspv.com` and `https://backend.xxx.projects.brc100.app` are **different origins**

2. **Cross-Origin Request**: When JavaScript on `microblog.bitspv.com` tries to fetch from `backend.xxx.projects.brc100.app`
   - Browser blocks it UNLESS the backend server says "yes, allow this"

3. **CORS Headers**: The server must send headers saying "I allow requests from this origin"
   - `Access-Control-Allow-Origin: https://microblog.bitspv.com` (or `*` for all)
   - `Access-Control-Allow-Methods: POST, GET, OPTIONS`
   - `Access-Control-Allow-Headers: Content-Type, Authorization`

## How CORS Works:

### Simple Requests (GET, POST with simple headers):
1. Browser sends request directly
2. Server responds with CORS headers
3. Browser checks headers - if allowed, JavaScript gets response; if not, browser blocks it

### Preflight Requests (POST with custom headers, PUT, DELETE, etc.):
1. Browser sends **OPTIONS** request first (preflight)
2. Server must respond with CORS headers on the OPTIONS request
3. If OPTIONS succeeds, browser sends actual request
4. If OPTIONS fails (404, no CORS headers), browser blocks the actual request

## Your Current Issue:

**Request**: `https://microblog.bitspv.com` → `https://backend.40406d9ea258f56b1f358c1dacb53921.projects.brc100.app/lookup`

**What's Happening**:
1. Browser sends OPTIONS preflight request (because it's a POST with custom headers)
2. External backend returns **404 Not Found** on OPTIONS request
3. Browser blocks the actual POST request (CORS error)

**Why This Happens**:
- The external backend service doesn't support CORS properly
- It doesn't handle OPTIONS requests (returns 404)
- This is **NOT a problem with your wallet** - it's a problem with the external service

## Our Wallet's CORS Handling:

**For Wallet Endpoints** (intercepted requests):
- ✅ We set CORS headers in `HttpRequestInterceptor.cpp` (lines 283-286):
  ```cpp
  response->SetHeaderByName("Access-Control-Allow-Origin", "*", true);
  response->SetHeaderByName("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS", true);
  response->SetHeaderByName("Access-Control-Allow-Headers", "Content-Type, Authorization", true);
  ```
- ✅ Wallet endpoints work with CORS

**For External Backend Requests** (NOT intercepted):
- ❌ We don't intercept these (correct behavior)
- ❌ External backend must handle CORS itself
- ❌ If external backend doesn't support CORS, requests fail (not our problem)

## Security Implications:

**Why CORS Exists**:
- Prevents malicious websites from making requests to other sites on your behalf
- Protects user data from being accessed by unauthorized sites

**For Your Wallet**:
- ✅ Setting `Access-Control-Allow-Origin: *` is safe for localhost wallet endpoints
- ✅ Only intercepts requests to `localhost:3301` (local wallet)
- ✅ External requests go through normally (browser handles CORS)

**Best Practice**:
- For production, consider restricting `Access-Control-Allow-Origin` to specific domains
- For development/localhost, `*` is fine

## Summary:

**The CORS error you're seeing is NOT a wallet issue** - it's the external backend service (`backend.xxx.projects.brc100.app`) that doesn't support CORS properly. The microblog app is trying to use an external service that doesn't allow cross-origin requests.

**What You Can Do**:
1. ✅ Nothing - this is the external service's problem
2. ✅ Verify wallet endpoints work (they do - CORS headers are set)
3. ✅ Note that external backend services must handle their own CORS

**Your wallet is working correctly** - the CORS error is from an external service that microblog.bitspv.com is trying to use.
