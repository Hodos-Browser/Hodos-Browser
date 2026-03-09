import React, { useState, useEffect, useCallback } from 'react';

interface Certificate {
  type: string;
  serial_number: string;
  subject: string;
  certifier: string;
  revocation_outpoint: string;
  signature: string;
  fields: Record<string, string>;
  keyring: Record<string, string>;
}

const CertificatesTab: React.FC = () => {
  const [certificates, setCertificates] = useState<Certificate[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchCertificates = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const res = await fetch('http://localhost:31301/listCertificates', {
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

  const truncateHash = (hash: string): string => {
    if (!hash || hash.length <= 16) return hash;
    return `${hash.substring(0, 8)}...${hash.substring(hash.length - 8)}`;
  };

  const decodeType = (typeB64: string): string => {
    try {
      return atob(typeB64);
    } catch {
      return typeB64 || 'N/A';
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
          </span>
        </div>
      ) : (
        <>
          <span style={{ fontSize: '13px', color: '#6b7280' }}>
            {certificates.length} certificate{certificates.length !== 1 ? 's' : ''}
          </span>

          <table className="wd-cert-table">
            <thead>
              <tr>
                <th>Type</th>
                <th>Certifier</th>
                <th>Subject</th>
                <th>Fields</th>
                <th>Serial</th>
              </tr>
            </thead>
            <tbody>
              {certificates.map((cert, idx) => (
                <tr key={idx}>
                  <td>{decodeType(cert.type)}</td>
                  <td><span className="wd-cert-hash">{truncateHash(cert.certifier)}</span></td>
                  <td><span className="wd-cert-hash">{truncateHash(cert.subject)}</span></td>
                  <td>{Object.keys(cert.fields).length} field(s)</td>
                  <td><span className="wd-cert-hash">{cert.serial_number ? truncateHash(cert.serial_number) : 'N/A'}</span></td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
};

export default CertificatesTab;
