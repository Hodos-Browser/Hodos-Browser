//! Certificate repository for database operations
//!
//! Handles CRUD operations for certificates and certificate fields in the database.

use rusqlite::{Connection, Result as SqliteResult, params};
use log::info;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::certificate::types::{Certificate, CertificateField};
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose::STANDARD};

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

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let created_at = if certificate.created_at == 0 { now } else { certificate.created_at };

        // Insert certificate
        let certificate_id = {
            self.conn.execute(
                "INSERT INTO certificates (
                    user_id, type, serial_number, certifier,
                    subject, verifier, revocation_outpoint, signature, is_deleted,
                    created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    certificate.user_id.unwrap_or(1),  // Default to user 1
                    STANDARD.encode(&certificate.type_),
                    STANDARD.encode(&certificate.serial_number),
                    hex::encode(&certificate.certifier),
                    hex::encode(&certificate.subject),
                    certificate.verifier.as_ref().map(|v| hex::encode(v)),
                    certificate.revocation_outpoint,
                    hex::encode(&certificate.signature),
                    certificate.is_deleted as i32,
                    created_at,
                    created_at,
                ],
            )?;

            self.conn.last_insert_rowid()
        };

        // Insert certificate fields
        let user_id = certificate.user_id.unwrap_or(1);
        for (field_name, field) in certificate.fields.iter_mut() {
            let field_created = if field.created_at == 0 { now } else { field.created_at };

            self.conn.execute(
                "INSERT INTO certificate_fields (
                    certificateId, user_id, field_name, field_value, master_key, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    certificate_id,
                    user_id,
                    field_name,
                    STANDARD.encode(&field.field_value),
                    STANDARD.encode(&field.master_key),
                    field_created,
                    field_created,
                ],
            )?;

            field.certificate_id = Some(certificate_id);
            field.user_id = Some(user_id);
        }

        certificate.certificate_id = Some(certificate_id);
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
            "SELECT certificateId, user_id, type, serial_number, certifier,
                    subject, verifier, revocation_outpoint, signature, is_deleted,
                    created_at, updated_at
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
                cert.fields = self.get_certificate_fields(cert.certificate_id.unwrap())?;
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
            "SELECT certificateId, user_id, type, serial_number, certifier,
                    subject, verifier, revocation_outpoint, signature, is_deleted,
                    created_at, updated_at
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

        query.push_str(" ORDER BY created_at DESC");

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
            if let Some(cert_id) = cert.certificate_id {
                cert.fields = self.get_certificate_fields(cert_id)?;
            }
            certificates.push(cert);
        }

        Ok(certificates)
    }

    /// Get certificate fields for a certificate ID
    pub fn get_certificate_fields(&self, certificate_id: i64) -> SqliteResult<HashMap<String, CertificateField>> {
        let mut stmt = self.conn.prepare(
            "SELECT certificateId, user_id, field_name, field_value, master_key, created_at, updated_at
             FROM certificate_fields
             WHERE certificateId = ?1"
        )?;

        let field_iter = stmt.query_map(
            params![certificate_id],
            |row| {
                Ok(CertificateField {
                    certificate_id: Some(row.get(0)?),
                    user_id: row.get(1)?,
                    field_name: row.get(2)?,
                    field_value: STANDARD.decode(row.get::<_, String>(3)?)
                        .map_err(|e| rusqlite::Error::InvalidColumnType(3, format!("Invalid base64: {}", e), rusqlite::types::Type::Text))?,
                    master_key: STANDARD.decode(row.get::<_, String>(4)?)
                        .map_err(|e| rusqlite::Error::InvalidColumnType(4, format!("Invalid base64: {}", e), rusqlite::types::Type::Text))?,
                    created_at: row.get(5)?,
                    updated_at: row.get(6)?,
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
    /// Sets `is_deleted = true` and `updated_at = NOW()`.
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
             SET is_deleted = 1, updated_at = ?1
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

    /// Update publish status for a certificate
    ///
    /// Sets `publish_status`, `publish_txid`, `publish_vout`, and `updated_at`.
    pub fn update_publish_status(
        &self,
        type_: &[u8],
        serial_number: &[u8],
        certifier: &[u8],
        publish_status: &str,
        publish_txid: Option<&str>,
        publish_vout: Option<i32>,
    ) -> SqliteResult<bool> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let rows_affected = self.conn.execute(
            "UPDATE certificates
             SET publish_status = ?1, publish_txid = ?2, publish_vout = ?3, updated_at = ?4
             WHERE type = ?5 AND serial_number = ?6 AND certifier = ?7",
            params![
                publish_status,
                publish_txid,
                publish_vout,
                now,
                STANDARD.encode(type_),
                STANDARD.encode(serial_number),
                hex::encode(certifier),
            ],
        )?;

        Ok(rows_affected > 0)
    }

    /// Get publish info for a certificate
    ///
    /// Returns `(publish_status, publish_txid, publish_vout)`.
    pub fn get_publish_info(
        &self,
        type_: &[u8],
        serial_number: &[u8],
        certifier: &[u8],
    ) -> SqliteResult<Option<(String, Option<String>, Option<i32>)>> {
        let result = self.conn.query_row(
            "SELECT publish_status, publish_txid, publish_vout
             FROM certificates
             WHERE type = ?1 AND serial_number = ?2 AND certifier = ?3
             LIMIT 1",
            params![
                STANDARD.encode(type_),
                STANDARD.encode(serial_number),
                hex::encode(certifier),
            ],
            |row| {
                Ok((
                    row.get::<_, String>(0).unwrap_or_else(|_| "unpublished".to_string()),
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i32>>(2)?,
                ))
            },
        );

        match result {
            Ok(info) => Ok(Some(info)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Helper: Build certificate from database row
    ///
    /// Column order: certificateId(0), user_id(1), type(2), serial_number(3),
    /// certifier(4), subject(5), verifier(6), revocation_outpoint(7),
    /// signature(8), is_deleted(9), created_at(10), updated_at(11)
    fn build_certificate_from_row(
        &self,
        row: &rusqlite::Row,
        id: i64,
    ) -> SqliteResult<Certificate> {
        let verifier_hex: Option<String> = row.get(6)?;

        Ok(Certificate {
            certificate_id: Some(id),
            user_id: row.get(1)?,
            type_: STANDARD.decode(row.get::<_, String>(2)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(2, format!("Invalid base64: {}", e), rusqlite::types::Type::Text))?,
            serial_number: STANDARD.decode(row.get::<_, String>(3)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(3, format!("Invalid base64: {}", e), rusqlite::types::Type::Text))?,
            certifier: hex::decode(row.get::<_, String>(4)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(4, format!("Invalid hex: {}", e), rusqlite::types::Type::Text))?,
            subject: hex::decode(row.get::<_, String>(5)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(5, format!("Invalid hex: {}", e), rusqlite::types::Type::Text))?,
            verifier: verifier_hex.map(|h| hex::decode(h))
                .transpose()
                .map_err(|e| rusqlite::Error::InvalidColumnType(6, format!("Invalid hex: {}", e), rusqlite::types::Type::Text))?,
            revocation_outpoint: row.get(7)?,
            signature: hex::decode(row.get::<_, String>(8)?)
                .map_err(|e| rusqlite::Error::InvalidColumnType(8, format!("Invalid hex: {}", e), rusqlite::types::Type::Text))?,
            fields: HashMap::new(),  // Will be loaded separately
            keyring: HashMap::new(),  // Will be populated from fields
            is_deleted: row.get::<_, i32>(9)? != 0,
            created_at: row.get(10)?,
            updated_at: row.get(11)?,
        })
    }
}
