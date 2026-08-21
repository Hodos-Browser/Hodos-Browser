import sqlite3, os
p = os.path.join(os.environ['APPDATA'], 'HodosBrowserDev', 'wallet', 'wallet.db')
c = sqlite3.connect('file:' + p.replace(os.sep, '/') + '?mode=ro', uri=True)
print('recent txs (last ~1500s):')
for r in c.execute("SELECT txid, status, description, created_at FROM transactions WHERE created_at > strftime('%s','now') - 1500 ORDER BY created_at DESC LIMIT 10"): print(' ', r[1], r[0][:20], '|', (r[2] or '')[:40])
