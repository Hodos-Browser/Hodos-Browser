//! Certificate repository for database operations
//!
//! Handles CRUD operations for certificates and certificate fields in the database.

use rusqlite::{Connection, Result as SqliteResult, params};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::certificate::types::{Certificate, CertificateField};
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Sha256, Digest};

pub struct CertificateRepository<'a> {
    conn: &'a Connection,
}

impl<'a> CertificateRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        CertificateRepository { conn }
    }

    /// Insert certificate with fields (transaction)
    ///
    /// Inserts certificate and all its fields in a single transaction.
    /// Returns the certificate ID.
    pub fn insert_certificate_with_fields(
        &self,
        certificate: &mut Certificate,
    ) -> SqliteResult<i64> {
        info!("   Inserting certificate: type={}, serial={}, certifier={}",
            STANDARD.encode(&certificate.type_),
            STANDARD.encode(&certificate.serial_number),
            hex::encode(&certificate.certifier));

        // Insert certificate (without transaction for now - Connection is &self, not &mut)
        // TODO: Consider restructuring to support transactions
        let certificate_id = {
            let created_at = if certificate.acquired_at == 0 {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
            } else {
                certificate.acquired_at
            };

            // Workaround: If certificate_txid is None, generate a placeholder based on certificate data
            // This is needed because existing databases may have NOT NULL constraint on certificate_txid
            // For certificates acquired via issuance protocol, there's no transaction ID yet
            let certificate_txid = certificate.certificate_txid.clone().unwrap_or_else(|| {
                // Generate a unique placeholder: hash of type + serial_number
                let mut hasher = Sha256::new();
                hasher.update(&certificate.type_);
                hasher.update(&certificate.serial_number);
                let hash = hasher.finalize();
                format!("Not on Chain_{}", hex::encode(&hash[..16])) // Use first 16 bytes for shorter ID
            });

            self.conn.execute(
                "INSERT INTO certificates (
                    certificate_txid, identity_key, type, serial_number, certifier,
                    subject, verifier, revocation_outpoint, signature, is_deleted, acquired_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    certificate_txid,
                    hex::encode(&certificate.subject),  // identity_key for backward compatibility
                    STANDARD.encode(&certificate.type_),
                    STANDARD.encode(&certificate.serial_number),
                    hex::encode(&certificate.certifier),
                    hex::encode(&certificate.subject),
                    certificate.verifier.as_ref().map(|v| hex::encode(v)),
                    certificate.revocation_outpoint,
                    hex::encode(&certificate.signature),
                    certificate.is_deleted as i32,
                    created_at,
                ],
            )?;

            self.conn.last_insert_rowid()
        };

        // Insert certificate fields
        for (field_name, field) in certificate.fields.iter_mut() {
            let created_at = if field.created_at == 0 {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
            } else {
                field.created_at
            };

            self.conn.execute(
                "INSERT INTO certificate_fields (
                    certificate_id, field_name, field_value, master_key, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    certificate_id,
                    field_name,
                    STANDARD.encode(&field.field_value),
                    STANDARD.encode(&field.master_key),
                    created_at,
                    created_at,
                ],
            )?;

            field.id = Some(certificate_id);  // Store certificate_id in field
            field.certificate_id = Some(certificate_id);
        }

        // No transaction to commit (inserts done directly)
        certificate.id = Some(certificate_id);
        info!("   ✅ Certificate inserted with ID: {}", certificate_id);

        Ok(certificate_id)
    }

    /// Get certificate by identifiers (type, serialNumber, certifier)
    ///
    /// Returns the certificate with all its fields.
    pub fn get_by_identifiers(
        &self,
        type_: &[u8],
        serial_number: &[u8],
        certifier: &[u8],
    ) -> SqliteResult<Option<Certificate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, certificate_txid, identity_key, type, serial_number, certifier,
                    subject, verifier, revocation_outpoint, signature, is_deleted,
                    acquired_at, relinquished_at
             FROM certificates
             WHERE type = ?1 AND serial_number = ?2 AND certifier = ?3
             LIMIT 1"
        )?;

        let cert_result = stmt.query_row(
            params![
                STANDARD.encode(type_),
                STANDARD.encode(serial_number),
                hex::encode(certifier),
            ],
            |row| {
                let id: i64 = row.get(0)?;
                let certificate = self.build_certificate_from_row(row, id)?;
                Ok(certificate)
            },
        );

        match cert_result {
            Ok(mut cert) => {
                // Load fields
                cert.fields = self.get_certificate_fields(cert.id.unwrap())?;
                Ok(Some(cert))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get certificate by transaction ID
    pub fn get_by_txid(&self, txid: &str) -> SqliteResult<Option<Certificate>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, certificate_txid, identity_key, type, serial_number, certifier,
                    subject, verifier, revocation_outpoint, signature, is_deleted,
                    acquired_at, relinquished_at
             FROM certificates
             WHERE certificate_txid = ?1
             LIMIT 1"
        )?;

        let cert_result = stmt.query_row(
            params![txid],
            |row| {
                let id: i64 = row.get(0)?;
                let certificate = self.build_certificate_from_row(row, id)?;
                Ok(certificate)
            },
        );

        match cert_result {
            Ok(mut cert) => {
                // Load fields
                cert.fields = self.get_certificate_fields(cert.id.unwrap())?;
                Ok(Some(cert))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// List certificates with filters
    ///
    /// Supports filtering by type, certifier, subject, and is_deleted status.
    /// Supports pagination with limit and offset.
    pub fn list_certificates(
        &self,
        type_filter: Option<&str>,
        certifier_filter: Option<&str>,
        subject_filter: Option<&str>,
        is_deleted: Option<bool>,
        limit: Option<i32>,
        offset: Option<i32>,
    ) -> SqliteResult<Vec<Certificate>> {
        let mut query = String::from(
            "SELECT id, certificate_txid, identity_key, type, serial_number, certifier,
                    subject, verifier, revocation_outpoint, signature, is_deleted,
                    acquired_at, relinquished_at
             FROM certificates
             WHERE 1=1"
        );

        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(t) = type_filter {
            query.push_str(" AND type = ?");
            params_vec.push(Box::new(t.to_string()));
        }

        if let Some(c) = certifier_filter {
            query.push_str(" AND certifier = ?");
            params_vec.push(Box::new(c.to_string()));
        }

        if let Some(s) = subject_filter {
            query.push_str(" AND subject = ?");
            params_vec.push(Box::new(s.to_string()));
        }

        if let Some(deleted) = is_deleted {
            query.push_str(" AND is_deleted = ?");
            params_vec.push(Box::new(if deleted { 1 } else { 0 }));
        }

        query.push_str(" ORDER BY acquired_at DESC");

        if let Some(l) = limit {
            query.push_str(" LIMIT ?");
            params_vec.push(Box::new(l));
        }

        if let Some(o) = offset {
            query.push_str(" OFFSET ?");
            params_vec.push(Box::new(o));
        }

        let mut stmt = self.conn.prepare(&query)?;
        let cert_iter = stmt.query_map(
            rusqlite::params_from_iter(params_vec.iter()),
            |row| {
                let id: i64 = row.get(0)?;
                let certificate = self.build_certificate_from_row(row, id)?;
                Ok(certificate)
            },
        )?;

        let mut certificates = Vec::new();
        for cert_result in cert_iter {
            let mut cert = cert_result?;
            // Load fields for each certificate
            if let Some(cert_id) = cert.id {
                cert.fields = self.get_certificate_fields(cert_id)?;
            }
            certificates.push(cert);
        }

        Ok(certificates)
    }

    /// Get certificate fields for a certificate ID
    pub fn get_certificate_fields(&self, certificate_id: i64) -> SqliteResult<HashMap<String, CertificateField>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, field_name, field_value, master_key, created_at, updated_at
             FROM certificate_fields
             WHERE certificate_id = ?1"
        )?;

        let field_iter = stmt.query_map(
            params![certificate_id],
            |row| {
                Ok(CertificateField {
                    id: Some(row.get(0)?),
                    certificate_id: Some(certificate_id),
                    field_name: row.get(1)?,
                    field_value: STANDARD.decode(row.get::<_, String>(2)?)
                        .map_err(|e| rusqlite::Error::InvalidColumnType(2, format!("Invalid base64: {}", e), rusqlite::types::Type::Text))?,
                    master_key: STANDARD.decode(row.get::<_, String>(3)?)
                        .map_err(|e| rusqlite::Error::InvalidColumnType(3, format!("Invalid base64: {}", e), rusqlite::types::Type::Text))?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            },
        )?;

        let mut fields = HashMap::new();
        for field_result in field_iter {
            let field = field_result?;
            fields.insert(field.field_name.clone(), field);
        }

        Ok(fields)
    }

    /// Update certificate relinquished status
    ///
    /// Sets `is_deleted = true` and `relinquished_at = NOW()`.
    pub fn update_relinquished(
        &self,
        type_: &[u8],
        serial_number: &[u8],
        certifier: &[u8],
    ) -> SqliteResult<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE certificates
             SET is_deleted = 1, relinquished_at = ?1
             WHERE type = ?2 AND serial_number = ?3 AND certifier = ?4",
            params![
                now,
                STANDARD.encode(type_),
                STANDARD.encode(serial_number),
                hex::encode(certifier),
            ],
        )?;

        Ok(rows_affected > 0)
    }

    /// Helper: Build certificate from database row
    fn build_certificate_from_row(
        &self,
        row: &rusqlite::Row,
        id: i64,
    ) -> SqliteResult<Certificate> {
        let verifier_hex: Option<String> = row.get(7)?;
        let relinquished_at: Option<i64> = row.get(12)?;

        Ok(Certificate {
            id: Some(id),
            certificate_txid: row.get(1)?,
            type_: STANDARD.decode(row.get::<_, String>(3)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(3, format!("Invalid base64: {}", e), rusqlite::types::Type::Text))?,
            subject: hex::decode(row.get::<_, String>(6)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(6, format!("Invalid hex: {}", e), rusqlite::types::Type::Text))?,
            serial_number: STANDARD.decode(row.get::<_, String>(4)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(4, format!("Invalid base64: {}", e), rusqlite::types::Type::Text))?,
            certifier: hex::decode(row.get::<_, String>(5)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(5, format!("Invalid hex: {}", e), rusqlite::types::Type::Text))?,
            verifier: verifier_hex.map(|h| hex::decode(h))
                .transpose()
                .map_err(|e| rusqlite::Error::InvalidColumnType(7, format!("Invalid hex: {}", e), rusqlite::types::Type::Text))?,
            revocation_outpoint: row.get(8)?,
            signature: hex::decode(row.get::<_, String>(9)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(9, format!("Invalid hex: {}", e), rusqlite::types::Type::Text))?,
            fields: HashMap::new(),  // Will be loaded separately
            keyring: HashMap::new(),  // Will be populated from fields
            acquired_at: row.get(11)?,
            is_deleted: row.get::<_, i32>(10)? != 0,
            relinquished_at,
        })
    }
}
