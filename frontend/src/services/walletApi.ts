// walletApi.ts — first-party wallet UI transport.
//
// Bridge migration (development-docs/0.4.0/WALLET_UI_BRIDGE_MIGRATION.md):
// the first-party React wallet UI no longer talks to the Rust wallet over a
// hardcoded `http://127.0.0.1:31301` fetch. It routes through the C++
// `window.__hodos_walletCall` IPC bridge instead, so C++ owns the wallet port
// (dev 31401 / prod 31301) and the frontend never knows it — which is what lets
// the dev browser and the installed browser run at the SAME time. C++ also gates
// the call by the un-forgeable frame origin (first-party loopback origin → no
// `X-Requesting-Domain` → Rust treats it as internal, exactly like the old
// direct fetch). The bridge is injected on every internal/overlay page (see
// simple_render_process_handler.cpp + WALLET_CALL_BRIDGE_SCRIPT).
//
// `walletFetch(path, init)` is a drop-in for `fetch('http://127.0.0.1:31301' + path, init)`:
//   - same call shape (path with leading slash, standard RequestInit)
//   - returns a fetch-like Response (`ok`, `status`, `json()`, `text()`)
//   - a non-2xx wallet response resolves with `{ ok:false, status }` (NOT a throw),
//     so existing `if (!res.ok)` / `res.status` checks keep working unchanged
//   - a genuine transport/parse failure rejects, like fetch on a network error
//   - honors `init.signal`: rejects with an `AbortError` when aborted (the
//     debounced-typeahead call sites in TransactionForm rely on this)

type WalletCall = (
  method: string,
  endpoint: string,
  body: unknown,
  httpMethod: string,
) => Promise<any>;

declare global {
  interface Window {
    __hodos_walletCall?: WalletCall;
  }
}

export interface WalletResponse {
  ok: boolean;
  status: number;
  statusText: string;
  json(): Promise<any>;
  text(): Promise<string>;
}

function makeResponse(ok: boolean, status: number, data: any): WalletResponse {
  return {
    ok,
    status,
    // The bridge carries no HTTP reason-phrase; surface the wallet error message
    // instead (more useful than a generic phrase, and keeps `res.statusText`
    // call sites compiling/working).
    statusText: ok ? 'OK' : (data && data.error ? String(data.error) : 'Error'),
    json: async () => data,
    text: async () => (typeof data === 'string' ? data : JSON.stringify(data ?? '')),
  };
}

// Friendly diagnostic name for the bridge's logs ("/wallet/status?x=1" -> "wallet/status").
function diag(path: string): string {
  return path.replace(/^\//, '').split('?')[0] || path;
}

async function dispatch(path: string, init?: RequestInit): Promise<WalletResponse> {
  const call = window.__hodos_walletCall;
  if (typeof call !== 'function') {
    // First-party contexts always have the bridge injected. If it's missing we
    // are in a non-CEF/test context — fail loudly rather than silently reaching
    // for a hardcoded port (which would defeat the whole migration).
    throw new Error('[walletApi] window.__hodos_walletCall bridge unavailable');
  }
  const httpMethod = (init?.method || 'GET').toUpperCase();
  let body: unknown = {};
  if (init?.body != null) {
    body = typeof init.body === 'string' ? JSON.parse(init.body as string) : init.body;
  }
  try {
    const data = await call(diag(path), path, body, httpMethod);
    return makeResponse(true, 200, data);
  } catch (err: any) {
    // The bridge rejects on a non-2xx wallet response (err.body = the parsed
    // error envelope, err.status = body.status if present) AND on a genuine
    // transport/parse failure (no err.body). Map the former back to a resolved
    // fetch-like Response; rethrow the latter so callers see a network error.
    if (err && err.body !== undefined) {
      const status = typeof err.status === 'number' ? err.status : 400;
      return makeResponse(false, status, err.body);
    }
    throw err;
  }
}

export function walletFetch(path: string, init?: RequestInit): Promise<WalletResponse> {
  const signal = init?.signal;
  if (!signal) return dispatch(path, init);
  if (signal.aborted) return Promise.reject(new DOMException('Aborted', 'AbortError'));
  return new Promise<WalletResponse>((resolve, reject) => {
    const onAbort = () => reject(new DOMException('Aborted', 'AbortError'));
    signal.addEventListener('abort', onAbort, { once: true });
    dispatch(path, init)
      .then(resolve, reject)
      .finally(() => signal.removeEventListener('abort', onAbort));
  });
}
