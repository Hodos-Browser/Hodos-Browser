import React, { useState, useEffect, useCallback } from 'react';

interface Certificate {
  type: string;
  type_name: string;
  serial_number: string;
  subject: string;
  certifier: string;
  certifier_name: string;
  revocation_outpoint: string;
  signature: string;
  fields: Record<string, string>;
  keyring: Record<string, string>;
  decrypted_fields: Record<string, string>;
  publish_status: string; // "unpublished" or "published"
  publish_txid?: string;
  created_at: number;
}

const CertificatesTab: React.FC = () => {
  const [certificates, setCertificates] = useState<Certificate[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);
  const [expandedCert, setExpandedCert] = useState<number | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<Certificate | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [publishingCert, setPublishingCert] = useState<string | null>(null); // serial_number of cert being published/unpublished

  const fetchCertificates = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await fetch('http://127.0.0.1:31301/listCertificates', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ limit: 100, offset: 0 }),
      });
      if (!res.ok) throw new Error(`Failed to fetch: ${res.statusText}`);
      const data = await res.json();
      setCertificates(data.certificates || []);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load certificates');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchCertificates();
  }, [fetchCertificates]);

  // Auto-clear success message after 5 seconds
  useEffect(() => {
    if (success) {
      const timer = setTimeout(() => setSuccess(null), 5000);
      return () => clearTimeout(timer);
    }
  }, [success]);

  const getTypeIcon = (typeName: string): string => {
    switch (typeName) {
      case 'X (Twitter)': return '𝕏';
      case 'Email': return '✉';
      case 'Discord': return '💬';
      case 'Government ID': return '🪪';
      case 'Registrant': return '🏢';
      case 'CoolCert': return '✅';
      default: return '📜';
    }
  };

  const getPrimaryField = (cert: Certificate): string => {
    const df = cert.decrypted_fields;
    if (!df || Object.keys(df).length === 0) return '—';
    if (df.userName) return `@${df.userName}`;
    if (df.email) return df.email;
    if (df.discordUsername) return df.discordUsername;
    if (df.cool) return `cool: ${df.cool}`;
    if (df.name) return df.name;
    const firstKey = Object.keys(df)[0];
    return df[firstKey] || '—';
  };

  const formatDate = (timestamp: number): string => {
    if (!timestamp) return '—';
    const date = new Date(timestamp * 1000);
    return date.toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' });
  };

  const truncateHash = (hash: string): string => {
    if (!hash || hash.length <= 16) return hash;
    return `${hash.substring(0, 8)}...${hash.substring(hash.length - 8)}`;
  };

  const hasRealRevocationOutpoint = (cert: Certificate): boolean => {
    return cert.revocation_outpoint !== 'not supported.0' &&
           cert.revocation_outpoint !== '0000000000000000000000000000000000000000000000000000000000000000.0' &&
           cert.revocation_outpoint.length > 10;
  };

  const getPublishStatusBadge = (cert: Certificate) => {
    const status = cert.publish_status || 'unpublished';
    if (status === 'published') {
      return <span style={{ fontSize: '11px', padding: '2px 6px', borderRadius: '3px', background: 'rgba(34,197,94,0.15)', color: '#22c55e' }}>Public</span>;
    }
    return <span style={{ fontSize: '11px', padding: '2px 6px', borderRadius: '3px', background: 'rgba(107,114,128,0.15)', color: '#6b7280' }}>Private</span>;
  };

  const handlePublish = async (cert: Certificate) => {
    setPublishingCert(cert.serial_number);
    setError(null);
    setSuccess(null);

    try {
      const fieldNames = Object.keys(cert.decrypted_fields || {});

      const res = await fetch('http://127.0.0.1:31301/wallet/certificate/publish', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          type: cert.type,
          serial_number: cert.serial_number,
          certifier: cert.certifier,
          fields_to_reveal: fieldNames,
        }),
      });

      const data = await res.json();

      if (!res.ok) {
        throw new Error(data.error || `Publish failed (${res.status})`);
      }

      if (data.success) {
        setSuccess(`${cert.type_name} certificate is now public.`);
        fetchCertificates();
      } else {
        throw new Error(data.error || 'Publish failed');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to publish certificate');
    } finally {
      setPublishingCert(null);
    }
  };

  const handleUnpublish = async (cert: Certificate) => {
    setPublishingCert(cert.serial_number);
    setError(null);
    setSuccess(null);

    try {
      const res = await fetch('http://127.0.0.1:31301/wallet/certificate/unpublish', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          type: cert.type,
          serial_number: cert.serial_number,
          certifier: cert.certifier,
        }),
      });

      const data = await res.json();

      if (!res.ok) {
        throw new Error(data.error || `Unpublish failed (${res.status})`);
      }

      if (data.success) {
        setSuccess(`${cert.type_name} certificate is now private.`);
        fetchCertificates();
      } else {
        throw new Error(data.error || 'Unpublish failed');
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to unpublish certificate');
    } finally {
      setPublishingCert(null);
    }
  };

  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return;
    setDeleting(true);
    setError(null);

    try {
      const res = await fetch('http://127.0.0.1:31301/relinquishCertificate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          type: deleteTarget.type,
          serial_number: deleteTarget.serial_number,
          certifier: deleteTarget.certifier,
        }),
      });

      if (!res.ok) {
        const data = await res.json().catch(() => ({}));
        if (data.is_published) {
          throw new Error('This certificate is publicly visible on the BSV overlay. Unpublish it first before deleting.');
        }
        throw new Error(data.error || `Failed to delete (${res.status})`);
      }

      setSuccess(`${deleteTarget.type_name} certificate removed from wallet.`);
      setDeleteTarget(null);
      setExpandedCert(null);
      fetchCertificates();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete certificate');
    } finally {
      setDeleting(false);
    }
  };

  if (loading) {
    return (
      <div className="wd-loading">
        <div className="wd-spinner" />
        <span>Loading certificates...</span>
      </div>
    );
  }

  return (
    <div className="wd-certificates">
      {error && <div className="wd-error-banner">{error}</div>}
      {success && (
        <div style={{
          background: 'rgba(34, 197, 94, 0.1)',
          border: '1px solid rgba(34, 197, 94, 0.3)',
          borderRadius: '6px',
          padding: '8px 12px',
          marginBottom: '8px',
          fontSize: '13px',
          color: '#22c55e',
        }}>
          {success}
        </div>
      )}

      {/* Delete Confirmation Modal */}
      {deleteTarget && (
        <div style={{
          position: 'fixed',
          top: 0, left: 0, right: 0, bottom: 0,
          background: 'rgba(0,0,0,0.6)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          zIndex: 1000,
        }}>
          <div style={{
            background: '#1e1e1e',
            border: '1px solid #333',
            borderRadius: '8px',
            padding: '20px',
            maxWidth: '420px',
            width: '90%',
          }}>
            <h3 style={{ margin: '0 0 12px', color: '#e5e7eb', fontSize: '16px' }}>
              Delete Certificate?
            </h3>

            <div style={{ fontSize: '13px', lineHeight: '1.6', color: '#9ca3af', marginBottom: '16px' }}>
              <p style={{ margin: '0 0 8px' }}>
                This will remove the <strong style={{ color: '#e5e7eb' }}>
                  {getTypeIcon(deleteTarget.type_name)} {deleteTarget.type_name}
                </strong> certificate
                {deleteTarget.decrypted_fields?.userName && (
                  <> for <strong style={{ color: '#e5e7eb' }}>@{deleteTarget.decrypted_fields.userName}</strong></>
                )}
                {deleteTarget.decrypted_fields?.email && (
                  <> for <strong style={{ color: '#e5e7eb' }}>{deleteTarget.decrypted_fields.email}</strong></>
                )}
                {' '}from your wallet.
              </p>

              {deleteTarget.publish_status === 'published' && (
                <div style={{
                  background: 'rgba(239, 68, 68, 0.1)',
                  border: '1px solid rgba(239, 68, 68, 0.3)',
                  borderRadius: '4px',
                  padding: '8px',
                  marginBottom: '8px',
                  color: '#ef4444',
                  fontSize: '12px',
                }}>
                  This certificate is publicly visible. It will be unpublished automatically before deletion.
                </div>
              )}

              {hasRealRevocationOutpoint(deleteTarget) && (
                <div style={{
                  background: 'rgba(234, 179, 8, 0.1)',
                  border: '1px solid rgba(234, 179, 8, 0.3)',
                  borderRadius: '4px',
                  padding: '8px',
                  marginBottom: '8px',
                  color: '#eab308',
                  fontSize: '12px',
                }}>
                  This certificate has an on-chain record. Removing it from your wallet does not revoke it on the blockchain. The certifier ({deleteTarget.certifier_name}) may still have records of this certificate.
                </div>
              )}

              <p style={{ margin: '0', fontSize: '12px', color: '#6b7280' }}>
                You can re-acquire this certificate later from {deleteTarget.certifier_name} if needed.
              </p>
            </div>

            <div style={{ display: 'flex', gap: '8px', justifyContent: 'flex-end' }}>
              <button
                onClick={() => setDeleteTarget(null)}
                disabled={deleting}
                style={{
                  padding: '8px 16px',
                  background: '#333',
                  border: '1px solid #444',
                  borderRadius: '4px',
                  color: '#e5e7eb',
                  cursor: 'pointer',
                  fontSize: '13px',
                }}
              >
                Cancel
              </button>
              <button
                onClick={handleDeleteConfirm}
                disabled={deleting}
                style={{
                  padding: '8px 16px',
                  background: deleting ? '#666' : '#dc2626',
                  border: 'none',
                  borderRadius: '4px',
                  color: '#fff',
                  cursor: deleting ? 'default' : 'pointer',
                  fontSize: '13px',
                }}
              >
                {deleting ? 'Deleting...' : 'Delete'}
              </button>
            </div>
          </div>
        </div>
      )}

      {certificates.length === 0 ? (
        <div className="wd-empty">
          <span className="wd-empty-icon">{'\u26E8'}</span>
          <span className="wd-empty-text">No certificates found</span>
          <span className="wd-empty-sub">
            Identity certificates (BRC-52) will appear here when acquired from trusted certifiers.
            Visit <a href="#" onClick={(e) => { e.preventDefault(); (window as any).cefMessage?.send('tab_create', ['https://socialcert.net']); }}>socialcert.net</a> to get started.
          </span>
        </div>
      ) : (
        <>
          <span style={{ fontSize: '13px', color: '#6b7280', marginBottom: '8px', display: 'block' }}>
            {certificates.length} certificate{certificates.length !== 1 ? 's' : ''}
          </span>

          <table className="wd-cert-table">
            <thead>
              <tr>
                <th>Type</th>
                <th>Identity</th>
                <th>Certifier</th>
                <th>Status</th>
                <th>Issued</th>
                <th style={{ width: '80px' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {certificates.map((cert, idx) => (
                <React.Fragment key={idx}>
                  <tr
                    style={{ cursor: 'pointer' }}
                    onClick={() => setExpandedCert(expandedCert === idx ? null : idx)}
                  >
                    <td>
                      <span style={{ marginRight: '6px' }}>{getTypeIcon(cert.type_name)}</span>
                      {cert.type_name}
                    </td>
                    <td style={{ fontFamily: 'monospace', fontSize: '12px' }}>
                      {getPrimaryField(cert)}
                    </td>
                    <td>{cert.certifier_name}</td>
                    <td>{getPublishStatusBadge(cert)}</td>
                    <td style={{ fontSize: '12px', color: '#9ca3af' }}>{formatDate(cert.created_at)}</td>
                    <td>
                      <div style={{ display: 'flex', gap: '4px' }}>
                        {/* Publish/Unpublish button */}
                        {cert.publish_status === 'published' ? (
                          <button
                            title="Make private"
                            onClick={(e) => { e.stopPropagation(); handleUnpublish(cert); }}
                            disabled={publishingCert === cert.serial_number}
                            style={{
                              background: publishingCert === cert.serial_number ? 'rgba(234,179,8,0.1)' : 'none',
                              border: 'none',
                              cursor: publishingCert === cert.serial_number ? 'default' : 'pointer',
                              color: publishingCert === cert.serial_number ? '#eab308' : '#eab308',
                              fontSize: publishingCert === cert.serial_number ? '11px' : '13px',
                              padding: '4px 6px',
                              borderRadius: '4px',
                              opacity: publishingCert === cert.serial_number ? 0.8 : 1,
                              whiteSpace: 'nowrap',
                            }}
                            onMouseOver={(e) => { if (publishingCert !== cert.serial_number) e.currentTarget.style.background = 'rgba(234,179,8,0.1)'; }}
                            onMouseOut={(e) => { if (publishingCert !== cert.serial_number) e.currentTarget.style.background = 'none'; }}
                          >
                            {publishingCert === cert.serial_number ? 'Unpublishing...' : '🔒'}
                          </button>
                        ) : (
                          <button
                            title="Make public"
                            onClick={(e) => { e.stopPropagation(); handlePublish(cert); }}
                            disabled={publishingCert === cert.serial_number}
                            style={{
                              background: publishingCert === cert.serial_number ? 'rgba(34,197,94,0.1)' : 'none',
                              border: 'none',
                              cursor: publishingCert === cert.serial_number ? 'default' : 'pointer',
                              color: publishingCert === cert.serial_number ? '#22c55e' : '#22c55e',
                              fontSize: publishingCert === cert.serial_number ? '11px' : '13px',
                              padding: '4px 6px',
                              borderRadius: '4px',
                              opacity: publishingCert === cert.serial_number ? 0.8 : 1,
                              whiteSpace: 'nowrap',
                            }}
                            onMouseOver={(e) => { if (publishingCert !== cert.serial_number) e.currentTarget.style.background = 'rgba(34,197,94,0.1)'; }}
                            onMouseOut={(e) => { if (publishingCert !== cert.serial_number) e.currentTarget.style.background = 'none'; }}
                          >
                            {publishingCert === cert.serial_number ? 'Publishing...' : '🌐'}
                          </button>
                        )}
                        {/* Delete button */}
                        <button
                          title="Delete certificate"
                          onClick={(e) => { e.stopPropagation(); setDeleteTarget(cert); }}
                          style={{
                            background: 'none',
                            border: 'none',
                            cursor: 'pointer',
                            color: '#ef4444',
                            fontSize: '14px',
                            padding: '4px 6px',
                            borderRadius: '4px',
                          }}
                          onMouseOver={(e) => (e.currentTarget.style.background = 'rgba(239,68,68,0.1)')}
                          onMouseOut={(e) => (e.currentTarget.style.background = 'none')}
                        >
                          🗑
                        </button>
                      </div>
                    </td>
                  </tr>

                  {expandedCert === idx && (
                    <tr>
                      <td colSpan={6} style={{ padding: '12px 16px', background: 'rgba(255,255,255,0.03)' }}>
                        <div style={{ fontSize: '12px', lineHeight: '1.8' }}>
                          {/* Decrypted fields */}
                          {cert.decrypted_fields && Object.keys(cert.decrypted_fields).length > 0 && (
                            <div style={{ marginBottom: '8px' }}>
                              <strong style={{ color: '#d1d5db' }}>Fields:</strong>
                              {Object.entries(cert.decrypted_fields).map(([name, value]) => (
                                <div key={name} style={{ marginLeft: '12px', color: '#9ca3af' }}>
                                  <span style={{ color: '#6b7280' }}>{name}:</span>{' '}
                                  {name === 'profilePhoto' ? (
                                    <img
                                      src={value}
                                      alt="avatar"
                                      style={{ width: '24px', height: '24px', borderRadius: '50%', verticalAlign: 'middle', marginLeft: '4px' }}
                                      onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
                                    />
                                  ) : (
                                    <span style={{ color: '#e5e7eb' }}>{value}</span>
                                  )}
                                </div>
                              ))}
                            </div>
                          )}

                          {/* Publish status detail */}
                          <div style={{ marginBottom: '8px' }}>
                            <strong style={{ color: '#d1d5db' }}>Visibility:</strong>
                            <span style={{ marginLeft: '8px' }}>
                              {getPublishStatusBadge(cert)}
                            </span>
                            {cert.publish_txid && (
                              <span style={{ marginLeft: '8px', color: '#6b7280' }}>
                                tx: <span className="wd-cert-hash" title={cert.publish_txid}>{truncateHash(cert.publish_txid)}</span>
                              </span>
                            )}
                          </div>

                          {/* Technical details */}
                          <div style={{ color: '#6b7280' }}>
                            <div>
                              <span>Certifier: </span>
                              <span className="wd-cert-hash" title={cert.certifier}>{cert.certifier_name} ({truncateHash(cert.certifier)})</span>
                            </div>
                            <div>
                              <span>Serial: </span>
                              <span className="wd-cert-hash">{truncateHash(cert.serial_number)}</span>
                            </div>
                            <div>
                              <span>Revocation: </span>
                              <span className="wd-cert-hash">
                                {hasRealRevocationOutpoint(cert) ? (
                                  <span title={cert.revocation_outpoint}>{truncateHash(cert.revocation_outpoint)} (on-chain)</span>
                                ) : (
                                  'N/A'
                                )}
                              </span>
                            </div>
                          </div>
                        </div>
                      </td>
                    </tr>
                  )}
                </React.Fragment>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
};

export default CertificatesTab;
