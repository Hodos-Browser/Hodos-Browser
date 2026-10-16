"""Applies a SCRATCH patch — never commit the APPLIED patch. Env-gated fake broadcaster (see README).
HODOS_DEV=1 + HODOS_P2_FAKE_BROADCAST=temporary|permanent|hang replaces the broadcast chain.
Usage: python scratch_fake_broadcaster.py <rust-wallet dir> apply|revert"""
import sys, shutil, os
root, action = sys.argv[1], sys.argv[2]
p = os.path.join(root, 'src', 'services', 'mod.rs')
bak = p + '.p2scratch'
if action == 'revert':
    shutil.copyfile(bak, p); os.remove(bak); print('reverted'); sys.exit(0)
if os.path.exists(bak):
    sys.exit('already applied')
s = open(p, 'rb').read().decode('utf-8')
old = '''            broadcast: ProviderCollection::new(vec![arc_gp, arc_tl, gp_mapi, woc]),
        }
    }'''
assert s.count(old) == 1
new = '''            broadcast: match p2_scratch_fake_broadcaster() {
                Some(fake) => ProviderCollection::new(vec![fake]),
                None => ProviderCollection::new(vec![arc_gp, arc_tl, gp_mapi, woc]),
            },
        }
    }'''
s = s.replace(old, new, 1)
s += '''

// ⛔⛔ SCRATCH — B5-T1-P2 owner sitting only. NEVER COMMIT. ⛔⛔
fn p2_scratch_fake_broadcaster() -> Option<Arc<dyn IndexerProvider>> {
    if std::env::var("HODOS_DEV").ok().as_deref() != Some("1") { return None; }
    let mode = std::env::var("HODOS_P2_FAKE_BROADCAST").ok()?;
    log::warn!("⛔ SCRATCH fake broadcaster active: mode={}", mode);
    Some(Arc::new(P2ScratchFake { mode }))
}
struct P2ScratchFake { mode: String }
#[async_trait::async_trait]
impl IndexerProvider for P2ScratchFake {
    fn name(&self) -> &'static str { "P2_SCRATCH_FAKE" }
    fn supports(&self, op: ProviderOp) -> bool { op == ProviderOp::BroadcastBeef }
    async fn broadcast_beef(&self, _beef: &[u8]) -> Result<BroadcastResult, IndexerError> {
        match self.mode.as_str() {
            "permanent" => Err(IndexerError::ProviderStatus { provider: "P2_SCRATCH_FAKE", status: 400,
                body: "Missing inputs (scratch fake)".to_string() }),
            "hang" => { tokio::time::sleep(Duration::from_secs(120)).await;
                Err(IndexerError::Transport("scratch fake hung".to_string())) }
            _ => Err(IndexerError::Transport("scratch fake: connection refused".to_string())),
        }
    }
}
'''
shutil.copyfile(p, bak)
open(p, 'wb').write(s.encode('utf-8'))
print('applied')
