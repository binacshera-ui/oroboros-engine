//! # Write-Ahead Log (WAL)
//!
//! **Crash-Safe Transaction System**
//!
//! Before any inventory modification, we write to the WAL.
//! If the server crashes mid-transaction, we can recover:
//! - Committed transactions: replay and apply
//! - Uncommitted transactions: rollback (do nothing)
//!
//! ## Guarantees
//!
//! 1. **Durability**: Once `commit()` returns, data is on disk
//! 2. **Atomicity**: Either all changes apply, or none do
//! 3. **Recovery**: On restart, incomplete transactions are rolled back
//!
//! ## Format
//!
//! ```text
//! [4 bytes: magic "OWAL"]
//! [4 bytes: version]
//! [8 bytes: last committed LSN]
//!
//! Entry format:
//! [8 bytes: LSN (Log Sequence Number)]
//! [1 byte: record type (BEGIN/OP/COMMIT/ROLLBACK)]
//! [4 bytes: payload length]
//! [N bytes: payload (serialized operation)]
//! [4 bytes: CRC32 of above]
//! ```

use crate::error::{EconomyError, EconomyResult};
use crate::inventory::ItemId;
use parking_lot::Mutex;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Magic bytes identifying a WAL file.
const WAL_MAGIC: &[u8; 4] = b"OWAL";

/// Current WAL format version.
const WAL_VERSION: u32 = 1;

/// WAL record types.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordType {
    /// Begin a new transaction.
    Begin = 1,
    /// An operation within a transaction.
    Operation = 2,
    /// Commit the transaction (durable).
    Commit = 3,
    /// Rollback the transaction.
    Rollback = 4,
}

impl RecordType {
    /// Converts from u8.
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            1 => Some(Self::Begin),
            2 => Some(Self::Operation),
            3 => Some(Self::Commit),
            4 => Some(Self::Rollback),
            _ => None,
        }
    }
}

/// Types of operations that can be logged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WalOperation {
    /// Add items to inventory.
    AddItem {
        /// Player/entity ID.
        entity_id: u64,
        /// Item being added.
        item_id: ItemId,
        /// Quantity added.
        quantity: u32,
    },
    /// Remove items from inventory.
    RemoveItem {
        /// Player/entity ID.
        entity_id: u64,
        /// Item being removed.
        item_id: ItemId,
        /// Quantity removed.
        quantity: u32,
    },
    /// Craft operation (atomic remove inputs + add outputs).
    Craft {
        /// Player/entity ID.
        entity_id: u64,
        /// Recipe ID.
        recipe_id: u32,
        /// Items consumed (for rollback).
        inputs: Vec<(ItemId, u32)>,
        /// Items produced.
        outputs: Vec<(ItemId, u32)>,
    },
    /// Mining loot drop.
    LootDrop {
        /// Player/entity ID.
        entity_id: u64,
        /// Block mined.
        block_id: u32,
        /// Item dropped.
        item_id: ItemId,
        /// Quantity dropped.
        quantity: u32,
    },
}

impl WalOperation {
    /// Serializes the operation to bytes.
    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        match self {
            Self::AddItem { entity_id, item_id, quantity } => {
                buf.push(1); // Type tag
                buf.extend_from_slice(&entity_id.to_le_bytes());
                buf.extend_from_slice(&item_id.to_le_bytes());
                buf.extend_from_slice(&quantity.to_le_bytes());
            }
            Self::RemoveItem { entity_id, item_id, quantity } => {
                buf.push(2);
                buf.extend_from_slice(&entity_id.to_le_bytes());
                buf.extend_from_slice(&item_id.to_le_bytes());
                buf.extend_from_slice(&quantity.to_le_bytes());
            }
            Self::Craft { entity_id, recipe_id, inputs, outputs } => {
                buf.push(3);
                buf.extend_from_slice(&entity_id.to_le_bytes());
                buf.extend_from_slice(&recipe_id.to_le_bytes());
                buf.extend_from_slice(&(inputs.len() as u32).to_le_bytes());
                for (item_id, qty) in inputs {
                    buf.extend_from_slice(&item_id.to_le_bytes());
                    buf.extend_from_slice(&qty.to_le_bytes());
                }
                buf.extend_from_slice(&(outputs.len() as u32).to_le_bytes());
                for (item_id, qty) in outputs {
                    buf.extend_from_slice(&item_id.to_le_bytes());
                    buf.extend_from_slice(&qty.to_le_bytes());
                }
            }
            Self::LootDrop { entity_id, block_id, item_id, quantity } => {
                buf.push(4);
                buf.extend_from_slice(&entity_id.to_le_bytes());
                buf.extend_from_slice(&block_id.to_le_bytes());
                buf.extend_from_slice(&item_id.to_le_bytes());
                buf.extend_from_slice(&quantity.to_le_bytes());
            }
        }

        buf
    }

    /// Deserializes an operation from bytes.
    fn deserialize(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let tag = data[0];
        let rest = &data[1..];

        match tag {
            1 if rest.len() >= 16 => {
                let entity_id = u64::from_le_bytes(rest[0..8].try_into().ok()?);
                let item_id = u32::from_le_bytes(rest[8..12].try_into().ok()?);
                let quantity = u32::from_le_bytes(rest[12..16].try_into().ok()?);
                Some(Self::AddItem { entity_id, item_id, quantity })
            }
            2 if rest.len() >= 16 => {
                let entity_id = u64::from_le_bytes(rest[0..8].try_into().ok()?);
                let item_id = u32::from_le_bytes(rest[8..12].try_into().ok()?);
                let quantity = u32::from_le_bytes(rest[12..16].try_into().ok()?);
                Some(Self::RemoveItem { entity_id, item_id, quantity })
            }
            4 if rest.len() >= 20 => {
                let entity_id = u64::from_le_bytes(rest[0..8].try_into().ok()?);
                let block_id = u32::from_le_bytes(rest[8..12].try_into().ok()?);
                let item_id = u32::from_le_bytes(rest[12..16].try_into().ok()?);
                let quantity = u32::from_le_bytes(rest[16..20].try_into().ok()?);
                Some(Self::LootDrop { entity_id, block_id, item_id, quantity })
            }
            // TODO: Implement Craft deserialization
            _ => None,
        }
    }
}

/// A WAL record on disk.
#[derive(Clone, Debug)]
pub struct WalRecord {
    /// Log Sequence Number (unique, monotonic).
    pub lsn: u64,
    /// Record type.
    pub record_type: RecordType,
    /// Payload data.
    pub payload: Vec<u8>,
}

/// Transaction handle for grouping operations.
pub struct Transaction<'a> {
    /// Reference to the WAL.
    wal: &'a WriteAheadLog,
    /// Transaction ID (LSN of BEGIN record).
    pub txn_id: u64,
    /// Operations in this transaction.
    operations: Vec<WalOperation>,
    /// Whether this transaction has been finalized.
    finalized: bool,
}

impl<'a> Transaction<'a> {
    /// Adds an operation to the transaction.
    pub fn add_operation(&mut self, op: WalOperation) -> EconomyResult<()> {
        if self.finalized {
            return Err(EconomyError::TransactionRolledBack {
                reason: "Transaction already finalized".to_string(),
            });
        }

        // Write operation to WAL
        self.wal.write_record(RecordType::Operation, &op.serialize())?;
        self.operations.push(op);

        Ok(())
    }

    /// Commits the transaction (durable).
    ///
    /// After this returns, the data is guaranteed to be on disk.
    pub fn commit(mut self) -> EconomyResult<Vec<WalOperation>> {
        if self.finalized {
            return Err(EconomyError::TransactionRolledBack {
                reason: "Transaction already finalized".to_string(),
            });
        }

        self.wal.write_record(RecordType::Commit, &[])?;
        self.wal.sync()?;
        self.finalized = true;

        Ok(std::mem::take(&mut self.operations))
    }

    /// Rolls back the transaction.
    pub fn rollback(mut self) -> EconomyResult<()> {
        if self.finalized {
            return Ok(());
        }

        self.wal.write_record(RecordType::Rollback, &[])?;
        self.finalized = true;

        Ok(())
    }
}

impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        // If not finalized, auto-rollback
        if !self.finalized {
            let _ = self.wal.write_record(RecordType::Rollback, &[]);
        }
    }
}

/// Write-Ahead Log for crash-safe transactions.
pub struct WriteAheadLog {
    /// Path to the WAL file.
    path: PathBuf,
    /// Current Log Sequence Number.
    current_lsn: AtomicU64,
    /// File handle (protected by mutex for writes).
    file: Mutex<BufWriter<File>>,
}

impl WriteAheadLog {
    /// Opens or creates a WAL file.
    ///
    /// If the file exists, it will be recovered (uncommitted transactions rolled back).
    pub fn open(path: impl AsRef<Path>) -> EconomyResult<Self> {
        let path = path.as_ref().to_path_buf();

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| EconomyError::InvalidConfig(format!("Failed to open WAL: {e}")))?;

        let mut wal = Self {
            path,
            current_lsn: AtomicU64::new(0),
            file: Mutex::new(BufWriter::new(file)),
        };

        // If file is empty, write header
        {
            let mut file = wal.file.lock();
            let metadata = file.get_ref().metadata()
                .map_err(|e| EconomyError::InvalidConfig(format!("Failed to get metadata: {e}")))?;

            if metadata.len() == 0 {
                file.write_all(WAL_MAGIC)
                    .map_err(|e| EconomyError::InvalidConfig(format!("Failed to write magic: {e}")))?;
                file.write_all(&WAL_VERSION.to_le_bytes())
                    .map_err(|e| EconomyError::InvalidConfig(format!("Failed to write version: {e}")))?;
                file.write_all(&0u64.to_le_bytes())
                    .map_err(|e| EconomyError::InvalidConfig(format!("Failed to write LSN: {e}")))?;
                file.flush()
                    .map_err(|e| EconomyError::InvalidConfig(format!("Failed to flush: {e}")))?;
            }
        }

        // Recover from existing WAL
        wal.recover()?;

        Ok(wal)
    }

    /// Begins a new transaction.
    pub fn begin_transaction(&self) -> EconomyResult<Transaction<'_>> {
        let lsn = self.write_record(RecordType::Begin, &[])?;

        Ok(Transaction {
            wal: self,
            txn_id: lsn,
            operations: Vec::new(),
            finalized: false,
        })
    }

    /// Writes a record to the WAL.
    fn write_record(&self, record_type: RecordType, payload: &[u8]) -> EconomyResult<u64> {
        let lsn = self.current_lsn.fetch_add(1, Ordering::SeqCst);

        let mut file = self.file.lock();

        // Write LSN
        file.write_all(&lsn.to_le_bytes())
            .map_err(|e| EconomyError::InvalidConfig(format!("WAL write failed: {e}")))?;

        // Write record type
        file.write_all(&[record_type as u8])
            .map_err(|e| EconomyError::InvalidConfig(format!("WAL write failed: {e}")))?;

        // Write payload length
        file.write_all(&(payload.len() as u32).to_le_bytes())
            .map_err(|e| EconomyError::InvalidConfig(format!("WAL write failed: {e}")))?;

        // Write payload
        file.write_all(payload)
            .map_err(|e| EconomyError::InvalidConfig(format!("WAL write failed: {e}")))?;

        // Calculate and write CRC32
        let mut crc_data = Vec::with_capacity(8 + 1 + 4 + payload.len());
        crc_data.extend_from_slice(&lsn.to_le_bytes());
        crc_data.push(record_type as u8);
        crc_data.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        crc_data.extend_from_slice(payload);

        let crc = crc32fast::hash(&crc_data);
        file.write_all(&crc.to_le_bytes())
            .map_err(|e| EconomyError::InvalidConfig(format!("WAL write failed: {e}")))?;

        Ok(lsn)
    }

    /// Syncs the WAL to disk.
    fn sync(&self) -> EconomyResult<()> {
        let mut file = self.file.lock();
        file.flush()
            .map_err(|e| EconomyError::InvalidConfig(format!("WAL sync failed: {e}")))?;
        file.get_ref().sync_all()
            .map_err(|e| EconomyError::InvalidConfig(format!("WAL sync failed: {e}")))?;
        Ok(())
    }

    /// Recovers the WAL after a crash.
    ///
    /// Returns the list of committed operations that need to be replayed.
    fn recover(&mut self) -> EconomyResult<Vec<WalOperation>> {
        let mut committed_ops = Vec::new();

        // Reopen file for reading
        let file = File::open(&self.path)
            .map_err(|e| EconomyError::InvalidConfig(format!("Failed to open WAL for recovery: {e}")))?;
        let mut reader = BufReader::new(file);

        // Read and verify header
        let mut magic = [0u8; 4];
        if reader.read_exact(&mut magic).is_err() {
            return Ok(committed_ops); // Empty file
        }
        if &magic != WAL_MAGIC {
            return Err(EconomyError::InvalidConfig("Invalid WAL magic".to_string()));
        }

        let mut version_bytes = [0u8; 4];
        reader.read_exact(&mut version_bytes)
            .map_err(|e| EconomyError::InvalidConfig(format!("Failed to read version: {e}")))?;
        let version = u32::from_le_bytes(version_bytes);
        if version != WAL_VERSION {
            return Err(EconomyError::InvalidConfig(format!("Unsupported WAL version: {version}")));
        }

        let mut lsn_bytes = [0u8; 8];
        reader.read_exact(&mut lsn_bytes)
            .map_err(|e| EconomyError::InvalidConfig(format!("Failed to read LSN: {e}")))?;
        let last_committed_lsn = u64::from_le_bytes(lsn_bytes);

        // Track open transactions
        let mut open_transactions: std::collections::HashMap<u64, Vec<WalOperation>> =
            std::collections::HashMap::new();
        let mut max_lsn = last_committed_lsn;

        // Read records
        loop {
            let record = match Self::read_record(&mut reader) {
                Ok(r) => r,
                Err(_) => break, // End of file or corruption
            };

            max_lsn = max_lsn.max(record.lsn);

            match record.record_type {
                RecordType::Begin => {
                    open_transactions.insert(record.lsn, Vec::new());
                }
                RecordType::Operation => {
                    if let Some(op) = WalOperation::deserialize(&record.payload) {
                        // Find the most recent open transaction
                        if let Some((&txn_id, _)) = open_transactions.iter().next() {
                            if let Some(ops) = open_transactions.get_mut(&txn_id) {
                                ops.push(op);
                            }
                        }
                    }
                }
                RecordType::Commit => {
                    // Find and commit the most recent open transaction
                    if let Some((&txn_id, _)) = open_transactions.iter().next() {
                        if let Some(ops) = open_transactions.remove(&txn_id) {
                            committed_ops.extend(ops);
                        }
                    }
                }
                RecordType::Rollback => {
                    // Find and discard the most recent open transaction
                    if let Some((&txn_id, _)) = open_transactions.iter().next() {
                        open_transactions.remove(&txn_id);
                    }
                }
            }
        }

        // Update current LSN
        self.current_lsn.store(max_lsn + 1, Ordering::SeqCst);

        // Log uncommitted transactions (they're automatically rolled back)
        if !open_transactions.is_empty() {
            eprintln!(
                "WAL Recovery: {} uncommitted transactions rolled back",
                open_transactions.len()
            );
        }

        Ok(committed_ops)
    }

    /// Reads a single record from the WAL.
    fn read_record(reader: &mut BufReader<File>) -> EconomyResult<WalRecord> {
        let mut lsn_bytes = [0u8; 8];
        reader.read_exact(&mut lsn_bytes)
            .map_err(|e| EconomyError::InvalidConfig(format!("Read error: {e}")))?;
        let lsn = u64::from_le_bytes(lsn_bytes);

        let mut type_byte = [0u8; 1];
        reader.read_exact(&mut type_byte)
            .map_err(|e| EconomyError::InvalidConfig(format!("Read error: {e}")))?;
        let record_type = RecordType::from_u8(type_byte[0])
            .ok_or_else(|| EconomyError::InvalidConfig("Invalid record type".to_string()))?;

        let mut len_bytes = [0u8; 4];
        reader.read_exact(&mut len_bytes)
            .map_err(|e| EconomyError::InvalidConfig(format!("Read error: {e}")))?;
        let payload_len = u32::from_le_bytes(len_bytes) as usize;

        let mut payload = vec![0u8; payload_len];
        reader.read_exact(&mut payload)
            .map_err(|e| EconomyError::InvalidConfig(format!("Read error: {e}")))?;

        let mut crc_bytes = [0u8; 4];
        reader.read_exact(&mut crc_bytes)
            .map_err(|e| EconomyError::InvalidConfig(format!("Read error: {e}")))?;
        let stored_crc = u32::from_le_bytes(crc_bytes);

        // Verify CRC
        let mut crc_data = Vec::with_capacity(8 + 1 + 4 + payload_len);
        crc_data.extend_from_slice(&lsn_bytes);
        crc_data.push(type_byte[0]);
        crc_data.extend_from_slice(&len_bytes);
        crc_data.extend_from_slice(&payload);

        let computed_crc = crc32fast::hash(&crc_data);
        if stored_crc != computed_crc {
            return Err(EconomyError::InvalidConfig("CRC mismatch".to_string()));
        }

        Ok(WalRecord {
            lsn,
            record_type,
            payload,
        })
    }

    /// Truncates the WAL after checkpoint.
    ///
    /// Call this after persisting state to the main database.
    pub fn checkpoint(&self) -> EconomyResult<()> {
        let mut file = self.file.lock();

        // Seek to beginning and rewrite header only
        file.seek(SeekFrom::Start(0))
            .map_err(|e| EconomyError::InvalidConfig(format!("Seek failed: {e}")))?;

        file.write_all(WAL_MAGIC)
            .map_err(|e| EconomyError::InvalidConfig(format!("Write failed: {e}")))?;
        file.write_all(&WAL_VERSION.to_le_bytes())
            .map_err(|e| EconomyError::InvalidConfig(format!("Write failed: {e}")))?;

        let current_lsn = self.current_lsn.load(Ordering::SeqCst);
        file.write_all(&current_lsn.to_le_bytes())
            .map_err(|e| EconomyError::InvalidConfig(format!("Write failed: {e}")))?;

        // Truncate file after header
        file.get_ref().set_len(16)
            .map_err(|e| EconomyError::InvalidConfig(format!("Truncate failed: {e}")))?;

        file.flush()
            .map_err(|e| EconomyError::InvalidConfig(format!("Flush failed: {e}")))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_wal_path() -> PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("test_wal_{id}.wal"))
    }

    #[test]
    fn test_wal_create_and_open() {
        let path = temp_wal_path();
        {
            let _wal = WriteAheadLog::open(&path).unwrap();
        }
        assert!(path.exists());
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_wal_transaction_commit() {
        let path = temp_wal_path();
        {
            let wal = WriteAheadLog::open(&path).unwrap();
            let mut txn = wal.begin_transaction().unwrap();

            txn.add_operation(WalOperation::AddItem {
                entity_id: 1,
                item_id: 100,
                quantity: 5,
            }).unwrap();

            let ops = txn.commit().unwrap();
            assert_eq!(ops.len(), 1);
        }
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_wal_transaction_rollback() {
        let path = temp_wal_path();
        {
            let wal = WriteAheadLog::open(&path).unwrap();
            let mut txn = wal.begin_transaction().unwrap();

            txn.add_operation(WalOperation::AddItem {
                entity_id: 1,
                item_id: 100,
                quantity: 5,
            }).unwrap();

            txn.rollback().unwrap();
        }
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_wal_auto_rollback_on_drop() {
        let path = temp_wal_path();
        {
            let wal = WriteAheadLog::open(&path).unwrap();
            let mut txn = wal.begin_transaction().unwrap();

            txn.add_operation(WalOperation::AddItem {
                entity_id: 1,
                item_id: 100,
                quantity: 5,
            }).unwrap();

            // Drop without commit - should auto-rollback
        }
        fs::remove_file(&path).ok();
    }

    #[test]
    fn test_wal_recovery() {
        let path = temp_wal_path();

        // Write some committed transactions
        {
            let wal = WriteAheadLog::open(&path).unwrap();
            let mut txn = wal.begin_transaction().unwrap();
            txn.add_operation(WalOperation::AddItem {
                entity_id: 1,
                item_id: 100,
                quantity: 10,
            }).unwrap();
            txn.commit().unwrap();
        }

        // Reopen and verify recovery
        {
            let wal = WriteAheadLog::open(&path).unwrap();
            // Recovery happens in open(), we can verify by checking LSN
            assert!(wal.current_lsn.load(Ordering::SeqCst) > 0);
        }

        fs::remove_file(&path).ok();
    }
}
