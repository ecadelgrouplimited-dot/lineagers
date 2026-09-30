//! # Tamper-Evident Audit Log
//!
//! ## What This Enforces
//! - Every record is hash-chained to its predecessor (SHA-256)
//! - Every record is signed with an Ed25519 key (verifiable with the public key alone)
//! - Records are persisted as JSON Lines and flushed to disk before `append` returns
//! - A log file has exactly one writer at a time (OS file lock)
//!
//! ## What This Detects
//! - Modification of any field of any record
//! - Deletion, insertion, or reordering of records
//! - Records signed by a different key than the one trusted by the verifier
//! - Truncation of the tail, when verified against a previously published [`Checkpoint`]
//!
//! ## What This Cannot Prevent
//! A party holding the signing key can rewrite and re-sign an entire log. Truncation of
//! the tail is only detectable against a checkpoint published somewhere the writer cannot
//! reach (another system, a ticket, a transparency log). Publish checkpoints.
//!
//! ## Example
//!
//! ```no_run
//! use lineage::audit::{AuditKey, AuditLog, VerifyOptions, verify_file};
//! use serde_json::json;
//!
//! let key = AuditKey::load_or_create("audit.key").unwrap();
//! let mut log = AuditLog::open_or_create("agent.jsonl", key, "agent-7").unwrap();
//! log.append("agent-7", "tool_call", json!({"tool": "send_email", "to": "ops@example.com"})).unwrap();
//! let checkpoint = log.head();
//!
//! let report = verify_file("agent.jsonl", &VerifyOptions {
//!     public_key: Some(log.public_key_hex()),
//!     checkpoint: Some(checkpoint),
//! });
//! assert!(report.ok);
//! ```

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Domain separator mixed into every record hash.
const HASH_DOMAIN: &[u8] = b"lineage-audit-v1\n";
/// `prev_hash` of the genesis record.
pub const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// `kind` of the first record in every log.
pub const GENESIS_KIND: &str = "genesis";

#[derive(Debug)]
pub enum AuditError {
    Io(String),
    /// The key file or key material is invalid.
    Key(String),
    /// The existing log failed verification and will not be appended to.
    Corrupt(VerifyFailure),
    /// Another writer holds the lock on this log.
    Locked(PathBuf),
    /// A log already exists at this path.
    AlreadyExists(PathBuf),
    /// The log was created with a different signing key.
    KeyMismatch { expected: String, found: String },
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditError::Io(e) => write!(f, "io error: {}", e),
            AuditError::Key(e) => write!(f, "key error: {}", e),
            AuditError::Corrupt(failure) => write!(f, "log is corrupt: {}", failure),
            AuditError::Locked(path) => write!(f, "log is locked by another writer: {}", path.display()),
            AuditError::AlreadyExists(path) => write!(f, "log already exists: {}", path.display()),
            AuditError::KeyMismatch { expected, found } => {
                write!(f, "log was signed by key {} but writer holds key {}", expected, found)
            }
        }
    }
}

impl std::error::Error for AuditError {}

impl From<std::io::Error> for AuditError {
    fn from(e: std::io::Error) -> Self {
        AuditError::Io(e.to_string())
    }
}

/// Ed25519 signing key for audit logs.
///
/// Deliberately not `Clone`: a deployment should hold its key in one place.
pub struct AuditKey {
    signing: SigningKey,
}

impl AuditKey {
    /// Generates a new key from the operating system's CSPRNG.
    pub fn generate() -> Self {
        let mut secret = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret);
        AuditKey { signing: SigningKey::from_bytes(&secret) }
    }

    pub fn from_secret_bytes(secret: &[u8; 32]) -> Self {
        AuditKey { signing: SigningKey::from_bytes(secret) }
    }

    /// Parses a hex-encoded 32-byte secret key.
    pub fn from_secret_hex(hex_secret: &str) -> Result<Self, AuditError> {
        let bytes = hex::decode(hex_secret.trim()).map_err(|e| AuditError::Key(e.to_string()))?;
        let secret: [u8; 32] = bytes
            .try_into()
            .map_err(|_| AuditError::Key("secret key must be 32 bytes".to_string()))?;
        Ok(Self::from_secret_bytes(&secret))
    }

    /// Loads a hex-encoded secret key from `path`.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, AuditError> {
        let contents = fs::read_to_string(path.as_ref())?;
        Self::from_secret_hex(&contents)
    }

    /// Loads the key at `path`, or generates one and writes it there (mode 0600 on Unix).
    pub fn load_or_create(path: impl AsRef<Path>) -> Result<Self, AuditError> {
        let path = path.as_ref();
        if path.exists() {
            return Self::load(path);
        }
        let key = Self::generate();
        key.save(path)?;
        Ok(key)
    }

    /// Writes the secret key to a new file. Fails if the file already exists.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), AuditError> {
        let path = path.as_ref();
        create_parent_dir(path)?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                AuditError::AlreadyExists(path.to_path_buf())
            } else {
                AuditError::from(e)
            }
        })?;
        file.write_all(hex::encode(self.signing.to_bytes()).as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    /// Hex-encoded public key. Share this with anyone who needs to verify logs.
    pub fn public_key_hex(&self) -> String {
        hex::encode(self.signing.verifying_key().to_bytes())
    }

    fn sign(&self, message: &[u8]) -> String {
        hex::encode(self.signing.sign(message).to_bytes())
    }
}

/// Parses a hex-encoded Ed25519 public key.
pub fn parse_public_key(hex_key: &str) -> Result<VerifyingKey, AuditError> {
    let bytes = hex::decode(hex_key.trim()).map_err(|e| AuditError::Key(e.to_string()))?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| AuditError::Key("public key must be 32 bytes".to_string()))?;
    VerifyingKey::from_bytes(&bytes).map_err(|e| AuditError::Key(e.to_string()))
}

/// One signed, hash-chained entry in an audit log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub seq: u64,
    /// RFC 3339, UTC, microsecond precision.
    pub timestamp: String,
    pub actor: String,
    pub kind: String,
    pub payload: Value,
    pub prev_hash: String,
    pub hash: String,
    pub signature: String,
}

impl Record {
    /// Recomputes this record's hash from its content fields.
    pub fn compute_hash(&self) -> String {
        compute_hash(self.seq, &self.timestamp, &self.actor, &self.kind, &self.payload, &self.prev_hash)
    }
}

fn compute_hash(seq: u64, timestamp: &str, actor: &str, kind: &str, payload: &Value, prev_hash: &str) -> String {
    let content = json!({
        "seq": seq,
        "timestamp": timestamp,
        "actor": actor,
        "kind": kind,
        "payload": payload,
        "prev_hash": prev_hash,
    });
    let mut canonical = String::new();
    write_canonical(&content, &mut canonical);

    let mut hasher = Sha256::new();
    hasher.update(HASH_DOMAIN);
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

/// Serializes JSON with object keys sorted, so hashes do not depend on map ordering.
fn write_canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::String((*key).clone()).to_string());
                out.push(':');
                write_canonical(&map[*key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out);
            }
            out.push(']');
        }
        other => out.push_str(&other.to_string()),
    }
}

/// A published position in a log. Verifying against a checkpoint detects any
/// rewrite or truncation of history up to that point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub seq: u64,
    pub hash: String,
}

/// Append-only, signed, hash-chained log backed by a JSON Lines file.
pub struct AuditLog {
    path: PathBuf,
    file: File,
    key: AuditKey,
    head: Checkpoint,
    log_id: String,
}

impl AuditLog {
    /// Creates a new log at `path` and writes its genesis record. Fails if the file exists.
    pub fn create(path: impl AsRef<Path>, key: AuditKey, log_id: &str) -> Result<Self, AuditError> {
        let path = path.as_ref().to_path_buf();
        create_parent_dir(&path)?;
        let file = OpenOptions::new().append(true).create_new(true).open(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                AuditError::AlreadyExists(path.clone())
            } else {
                AuditError::from(e)
            }
        })?;
        lock(&file, &path)?;

        let mut log = AuditLog {
            path,
            file,
            key,
            head: Checkpoint { seq: 0, hash: GENESIS_PREV_HASH.to_string() },
            log_id: log_id.to_string(),
        };
        let public_key = log.key.public_key_hex();
        log.write_record(0, log_id, GENESIS_KIND, json!({ "log_id": log_id, "public_key": public_key }), GENESIS_PREV_HASH)?;
        Ok(log)
    }

    /// Opens an existing log for appending. The whole log is verified first against
    /// `key`'s public key; a log that fails verification is never appended to.
    pub fn open(path: impl AsRef<Path>, key: AuditKey) -> Result<Self, AuditError> {
        let path = path.as_ref().to_path_buf();
        let file = OpenOptions::new().append(true).open(&path)?;
        lock(&file, &path)?;

        let records = read_records(&path).map_err(AuditError::Corrupt)?;
        let genesis_key = genesis_public_key(&records).map_err(AuditError::Corrupt)?;
        if genesis_key != key.public_key_hex() {
            return Err(AuditError::KeyMismatch { expected: genesis_key, found: key.public_key_hex() });
        }
        let verifying = parse_public_key(&genesis_key)?;
        let head = verify_records(&records, &verifying).map_err(AuditError::Corrupt)?;
        let log_id = records[0].payload["log_id"].as_str().unwrap_or_default().to_string();

        Ok(AuditLog { path, file, key, head, log_id })
    }

    /// Opens the log at `path` if it exists, otherwise creates it.
    pub fn open_or_create(path: impl AsRef<Path>, key: AuditKey, log_id: &str) -> Result<Self, AuditError> {
        if path.as_ref().exists() {
            Self::open(path, key)
        } else {
            Self::create(path, key, log_id)
        }
    }

    /// Appends a record. When this returns `Ok`, the record is on disk.
    pub fn append(&mut self, actor: &str, kind: &str, payload: Value) -> Result<Record, AuditError> {
        let seq = self.head.seq + 1;
        let prev_hash = self.head.hash.clone();
        self.write_record(seq, actor, kind, payload, &prev_hash)
    }

    fn write_record(&mut self, seq: u64, actor: &str, kind: &str, payload: Value, prev_hash: &str) -> Result<Record, AuditError> {
        let timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Micros, true);
        let hash = compute_hash(seq, &timestamp, actor, kind, &payload, prev_hash);
        let hash_bytes = hex::decode(&hash).expect("hash is hex");
        let record = Record {
            seq,
            timestamp,
            actor: actor.to_string(),
            kind: kind.to_string(),
            payload,
            prev_hash: prev_hash.to_string(),
            signature: self.key.sign(&hash_bytes),
            hash,
        };

        let mut line = serde_json::to_string(&record).map_err(|e| AuditError::Io(e.to_string()))?;
        line.push('\n');
        self.file.write_all(line.as_bytes())?;
        self.file.sync_data()?;

        self.head = Checkpoint { seq: record.seq, hash: record.hash.clone() };
        Ok(record)
    }

    /// The latest record's position. Publish it to make truncation detectable.
    pub fn head(&self) -> Checkpoint {
        self.head.clone()
    }

    pub fn log_id(&self) -> &str {
        &self.log_id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn public_key_hex(&self) -> String {
        self.key.public_key_hex()
    }

    /// Reads every record currently in the log.
    pub fn records(&self) -> Result<Vec<Record>, AuditError> {
        read_records(&self.path).map_err(AuditError::Corrupt)
    }
}

fn create_parent_dir(path: &Path) -> Result<(), AuditError> {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => Ok(fs::create_dir_all(parent)?),
        _ => Ok(()),
    }
}

fn lock(file: &File, path: &Path) -> Result<(), AuditError> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(fs::TryLockError::WouldBlock) => Err(AuditError::Locked(path.to_path_buf())),
        Err(fs::TryLockError::Error(e)) => Err(AuditError::from(e)),
    }
}

/// Why a log failed verification, and where.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifyFailure {
    /// 1-based line number in the file, when known.
    pub line: Option<usize>,
    pub seq: Option<u64>,
    pub reason: String,
}

impl fmt::Display for VerifyFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.line, self.seq) {
            (Some(line), Some(seq)) => write!(f, "line {} (seq {}): {}", line, seq, self.reason),
            (Some(line), None) => write!(f, "line {}: {}", line, self.reason),
            _ => write!(f, "{}", self.reason),
        }
    }
}

fn failure(line: Option<usize>, seq: Option<u64>, reason: impl Into<String>) -> VerifyFailure {
    VerifyFailure { line, seq, reason: reason.into() }
}

/// Reads and parses every record in a log file, without verifying it.
pub fn read_records(path: impl AsRef<Path>) -> Result<Vec<Record>, VerifyFailure> {
    let file = File::open(path.as_ref()).map_err(|e| failure(None, None, format!("cannot open log: {}", e)))?;
    let mut records = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| failure(Some(index + 1), None, format!("read error: {}", e)))?;
        let record: Record = serde_json::from_str(&line)
            .map_err(|e| failure(Some(index + 1), None, format!("malformed record: {}", e)))?;
        records.push(record);
    }
    if records.is_empty() {
        return Err(failure(None, None, "log is empty"));
    }
    Ok(records)
}

fn genesis_public_key(records: &[Record]) -> Result<String, VerifyFailure> {
    let genesis = &records[0];
    if genesis.kind != GENESIS_KIND {
        return Err(failure(Some(1), Some(genesis.seq), "first record is not a genesis record"));
    }
    genesis.payload["public_key"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| failure(Some(1), Some(genesis.seq), "genesis record has no public_key"))
}

/// Checks chain linkage, hashes, and signatures. Returns the head on success.
fn verify_records(records: &[Record], key: &VerifyingKey) -> Result<Checkpoint, VerifyFailure> {
    let mut prev_hash = GENESIS_PREV_HASH.to_string();
    for (index, record) in records.iter().enumerate() {
        let line = Some(index + 1);
        let seq = Some(record.seq);
        if record.seq != index as u64 {
            return Err(failure(line, seq, format!("expected seq {}, found {}", index, record.seq)));
        }
        if index > 0 && record.kind == GENESIS_KIND {
            return Err(failure(line, seq, "genesis record after start of log"));
        }
        if record.prev_hash != prev_hash {
            return Err(failure(line, seq, "prev_hash does not match previous record"));
        }
        if record.compute_hash() != record.hash {
            return Err(failure(line, seq, "hash does not match record content"));
        }
        let signature = hex::decode(&record.signature)
            .ok()
            .and_then(|bytes| Signature::from_slice(&bytes).ok())
            .ok_or_else(|| failure(line, seq, "signature is malformed"))?;
        let hash_bytes = hex::decode(&record.hash).map_err(|_| failure(line, seq, "hash is malformed"))?;
        if key.verify(&hash_bytes, &signature).is_err() {
            return Err(failure(line, seq, "signature is invalid"));
        }
        prev_hash = record.hash.clone();
    }
    let last = records.last().expect("records is non-empty");
    Ok(Checkpoint { seq: last.seq, hash: last.hash.clone() })
}

/// What a verifier trusts.
#[derive(Debug, Clone, Default)]
pub struct VerifyOptions {
    /// Hex public key the log must be signed with. Without it, the log is only checked
    /// against the key it declares in its own genesis record (self-attested).
    pub public_key: Option<String>,
    /// A previously published checkpoint that must still be present in the log.
    pub checkpoint: Option<Checkpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub ok: bool,
    pub records: usize,
    pub log_id: Option<String>,
    pub public_key: Option<String>,
    /// True when the signing key was supplied by the verifier rather than read from the log.
    pub key_trusted: bool,
    pub head: Option<Checkpoint>,
    pub failure: Option<VerifyFailure>,
}

/// Verifies a log file end to end.
pub fn verify_file(path: impl AsRef<Path>, options: &VerifyOptions) -> VerifyReport {
    let mut report = VerifyReport {
        ok: false,
        records: 0,
        log_id: None,
        public_key: None,
        key_trusted: options.public_key.is_some(),
        head: None,
        failure: None,
    };

    let records = match read_records(path) {
        Ok(records) => records,
        Err(e) => {
            report.failure = Some(e);
            return report;
        }
    };
    report.records = records.len();
    report.log_id = records[0].payload["log_id"].as_str().map(str::to_string);

    match verify_records_with(&records, options) {
        Ok((public_key, head)) => {
            report.public_key = Some(public_key);
            report.head = Some(head);
            report.ok = true;
        }
        Err(e) => report.failure = Some(e),
    }
    report
}

/// Verifies in-memory records (e.g. fetched over an API) with the same rules as [`verify_file`].
pub fn verify_records_with(records: &[Record], options: &VerifyOptions) -> Result<(String, Checkpoint), VerifyFailure> {
    if records.is_empty() {
        return Err(failure(None, None, "log is empty"));
    }
    let declared = genesis_public_key(records)?;
    let public_key = match &options.public_key {
        Some(trusted) => {
            if !trusted.trim().eq_ignore_ascii_case(&declared) {
                return Err(failure(Some(1), Some(0), "log declares a different public key than the trusted one"));
            }
            trusted.trim().to_lowercase()
        }
        None => declared,
    };
    let verifying = parse_public_key(&public_key).map_err(|e| failure(None, None, e.to_string()))?;
    let head = verify_records(records, &verifying)?;

    if let Some(checkpoint) = &options.checkpoint {
        match records.get(checkpoint.seq as usize) {
            Some(record) if record.hash == checkpoint.hash => {}
            Some(_) => {
                return Err(failure(None, Some(checkpoint.seq), "record at checkpoint has a different hash (history rewritten)"));
            }
            None => {
                return Err(failure(None, Some(checkpoint.seq), format!("log ends at seq {} before checkpoint (truncated)", head.seq)));
            }
        }
    }
    Ok((public_key, head))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let mut bytes = [0u8; 8];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            let dir = std::env::temp_dir().join(format!("lineage-audit-{}-{}", name, hex::encode(bytes)));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self, file: &str) -> PathBuf {
            self.0.join(file)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_log(path: &Path, key: AuditKey, events: usize) -> (String, Checkpoint) {
        let mut log = AuditLog::create(path, key, "test-log").unwrap();
        for i in 0..events {
            log.append("agent", "action", json!({ "i": i, "cost": 1.5, "tags": ["a", "b"] })).unwrap();
        }
        (log.public_key_hex(), log.head())
    }

    fn rewrite_lines(path: &Path, edit: impl FnOnce(&mut Vec<String>)) {
        let mut lines: Vec<String> = fs::read_to_string(path).unwrap().lines().map(str::to_string).collect();
        edit(&mut lines);
        fs::write(path, lines.join("\n") + "\n").unwrap();
    }

    fn trusted(public_key: &str) -> VerifyOptions {
        VerifyOptions { public_key: Some(public_key.to_string()), checkpoint: None }
    }

    #[test]
    fn test_roundtrip_verifies() {
        let dir = TempDir::new("roundtrip");
        let path = dir.path("log.jsonl");
        let (public_key, head) = write_log(&path, AuditKey::generate(), 5);

        let report = verify_file(&path, &VerifyOptions { public_key: Some(public_key), checkpoint: Some(head.clone()) });
        assert!(report.ok, "{:?}", report.failure);
        assert_eq!(report.records, 6);
        assert_eq!(report.head, Some(head));
        assert_eq!(report.log_id.as_deref(), Some("test-log"));
        assert!(report.key_trusted);
    }

    #[test]
    fn test_reopen_continues_chain() {
        let dir = TempDir::new("reopen");
        let path = dir.path("log.jsonl");
        let key_path = dir.path("audit.key");

        let key = AuditKey::load_or_create(&key_path).unwrap();
        write_log(&path, key, 2);

        let mut log = AuditLog::open(&path, AuditKey::load(&key_path).unwrap()).unwrap();
        assert_eq!(log.head().seq, 2);
        let record = log.append("agent", "action", json!({})).unwrap();
        assert_eq!(record.seq, 3);
        drop(log);

        assert!(verify_file(&path, &VerifyOptions::default()).ok);
    }

    #[test]
    fn test_second_writer_is_locked_out() {
        let dir = TempDir::new("lock");
        let path = dir.path("log.jsonl");
        let key_path = dir.path("audit.key");
        let _log = AuditLog::create(&path, AuditKey::load_or_create(&key_path).unwrap(), "x").unwrap();

        let second = AuditLog::open(&path, AuditKey::load(&key_path).unwrap());
        assert!(matches!(second, Err(AuditError::Locked(_))));
    }

    #[test]
    fn test_open_with_wrong_key_is_rejected() {
        let dir = TempDir::new("wrongkey");
        let path = dir.path("log.jsonl");
        write_log(&path, AuditKey::generate(), 1);

        let result = AuditLog::open(&path, AuditKey::generate());
        assert!(matches!(result, Err(AuditError::KeyMismatch { .. })));
    }

    #[test]
    fn test_modified_payload_is_detected() {
        let dir = TempDir::new("modify");
        let path = dir.path("log.jsonl");
        let (public_key, _) = write_log(&path, AuditKey::generate(), 3);

        rewrite_lines(&path, |lines| lines[2] = lines[2].replace("\"i\":1", "\"i\":99"));

        let report = verify_file(&path, &trusted(&public_key));
        assert!(!report.ok);
        let failure = report.failure.unwrap();
        assert_eq!(failure.line, Some(3));
        assert!(failure.reason.contains("hash"), "{}", failure.reason);
    }

    #[test]
    fn test_rehashed_record_without_key_is_detected() {
        // An attacker who edits content and recomputes the hash still cannot sign it.
        let dir = TempDir::new("rehash");
        let path = dir.path("log.jsonl");
        let (public_key, _) = write_log(&path, AuditKey::generate(), 1);

        rewrite_lines(&path, |lines| {
            let mut record: Record = serde_json::from_str(&lines[1]).unwrap();
            record.payload = json!({ "i": 42 });
            record.hash = record.compute_hash();
            lines[1] = serde_json::to_string(&record).unwrap();
        });

        let failure = verify_file(&path, &trusted(&public_key)).failure.unwrap();
        assert_eq!(failure.reason, "signature is invalid");
    }

    #[test]
    fn test_deleted_record_is_detected() {
        let dir = TempDir::new("delete");
        let path = dir.path("log.jsonl");
        let (public_key, _) = write_log(&path, AuditKey::generate(), 3);

        rewrite_lines(&path, |lines| {
            lines.remove(2);
        });

        assert!(!verify_file(&path, &trusted(&public_key)).ok);
    }

    #[test]
    fn test_reordered_records_are_detected() {
        let dir = TempDir::new("reorder");
        let path = dir.path("log.jsonl");
        let (public_key, _) = write_log(&path, AuditKey::generate(), 3);

        rewrite_lines(&path, |lines| lines.swap(1, 2));

        assert!(!verify_file(&path, &trusted(&public_key)).ok);
    }

    #[test]
    fn test_log_forged_with_another_key_is_detected() {
        let dir = TempDir::new("forged");
        let path = dir.path("log.jsonl");
        let real_key = AuditKey::generate();
        let real_public = real_key.public_key_hex();
        write_log(&path, AuditKey::generate(), 2);

        // Self-attested verification passes; trusting the real key does not.
        assert!(verify_file(&path, &VerifyOptions::default()).ok);
        let report = verify_file(&path, &trusted(&real_public));
        assert!(!report.ok);
        assert!(report.failure.unwrap().reason.contains("different public key"));
    }

    #[test]
    fn test_truncation_is_detected_with_checkpoint() {
        let dir = TempDir::new("truncate");
        let path = dir.path("log.jsonl");
        let (public_key, head) = write_log(&path, AuditKey::generate(), 4);

        rewrite_lines(&path, |lines| lines.truncate(3));

        // A truncated chain is still internally valid...
        assert!(verify_file(&path, &trusted(&public_key)).ok);
        // ...but not against the published checkpoint.
        let report = verify_file(&path, &VerifyOptions { public_key: Some(public_key), checkpoint: Some(head) });
        assert!(!report.ok);
        assert!(report.failure.unwrap().reason.contains("truncated"));
    }

    #[test]
    fn test_corrupt_log_is_never_appended_to() {
        let dir = TempDir::new("corrupt");
        let path = dir.path("log.jsonl");
        let key_path = dir.path("audit.key");
        write_log(&path, AuditKey::load_or_create(&key_path).unwrap(), 2);

        rewrite_lines(&path, |lines| lines[1] = lines[1].replace("agent", "someone-else"));

        let result = AuditLog::open(&path, AuditKey::load(&key_path).unwrap());
        assert!(matches!(result, Err(AuditError::Corrupt(_))));
    }

    #[test]
    fn test_hash_ignores_key_order() {
        let a: Value = serde_json::from_str(r#"{"b":1,"a":{"y":2,"x":[3,{"q":1,"p":2}]}}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"a":{"x":[3,{"p":2,"q":1}],"y":2},"b":1}"#).unwrap();
        assert_eq!(compute_hash(1, "t", "a", "k", &a, "p"), compute_hash(1, "t", "a", "k", &b, "p"));
    }

    #[test]
    fn test_key_file_roundtrip_and_no_overwrite() {
        let dir = TempDir::new("keyfile");
        let key_path = dir.path("keys/audit.key");
        let key = AuditKey::load_or_create(&key_path).unwrap();
        let loaded = AuditKey::load(&key_path).unwrap();
        assert_eq!(key.public_key_hex(), loaded.public_key_hex());
        assert!(matches!(AuditKey::generate().save(&key_path), Err(AuditError::AlreadyExists(_))));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&key_path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
    }
}
