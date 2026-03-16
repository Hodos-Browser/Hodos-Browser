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
  created_at: number;
}

const CertificatesTab: React.FC = () => {
  const [certificates, setCertificates] = useState<Certificate[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expandedCert, setExpandedCert] = useState<number | null>(null);

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

    // Return the most relevant field based on cert type
    if (df.userName) return `@${df.userName}`;
    if (df.email) return df.email;
    if (df.discordUsername) return df.discordUsername;
    if (df.cool) return `cool: ${df.cool}`;
    if (df.name) return df.name;

    // Fallback: first field
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

  const handleDelete = async (cert: Certificate) => {
    if (!confirm(`Delete this ${cert.type_name} certificate?\n\nThis will remove it from your wallet.`)) return;

    try {
      const res = await fetch('http://127.0.0.1:31301/relinquishCertificate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          type: cert.type,
          serial_number: cert.serial_number,
          certifier: cert.certifier,
        }),
      });
      if (!res.ok) throw new Error('Failed to delete certificate');
      fetchCertificates();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to delete');
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
                <th>Issued</th>
                <th style={{ width: '60px' }}>Actions</th>
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
                    <td style={{ fontSize: '12px', color: '#9ca3af' }}>{formatDate(cert.created_at)}</td>
                    <td>
                      <button
                        className="wd-cert-action-btn"
                        title="Delete certificate"
                        onClick={(e) => { e.stopPropagation(); handleDelete(cert); }}
                        style={{
                          background: 'none',
                          border: 'none',
                          cursor: 'pointer',
                          color: '#ef4444',
                          fontSize: '14px',
                          padding: '4px 8px',
                          borderRadius: '4px',
                        }}
                        onMouseOver={(e) => (e.currentTarget.style.background = 'rgba(239,68,68,0.1)')}
                        onMouseOut={(e) => (e.currentTarget.style.background = 'none')}
                      >
                        🗑
                      </button>
                    </td>
                  </tr>

                  {expandedCert === idx && (
                    <tr>
                      <td colSpan={5} style={{ padding: '12px 16px', background: 'rgba(255,255,255,0.03)' }}>
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

                          {/* Technical details */}
                          <div style={{ color: '#6b7280' }}>
                            <div>
                              <span>Certifier: </span>
                              <span className="wd-cert-hash" title={cert.certifier}>{truncateHash(cert.certifier)}</span>
                            </div>
                            <div>
                              <span>Serial: </span>
                              <span className="wd-cert-hash">{truncateHash(cert.serial_number)}</span>
                            </div>
                            <div>
                              <span>Revocation: </span>
                              <span className="wd-cert-hash">{cert.revocation_outpoint === 'not supported.0' ? 'N/A' : truncateHash(cert.revocation_outpoint)}</span>
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
