use generic_array::{typenum, GenericArray};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::{proof_of_work::ProofOfWork, transaction::Transaction};

#[derive(Debug, Error)]
pub enum BlockError {
    #[error("Failed to get system time: {0}")]
    TimeError(#[from] std::time::SystemTimeError),
    #[error("Failed to convert data to string: {0}")]
    DataEncodingError(#[from] std::string::FromUtf8Error),
    #[error("Invalid proof of work: {0}")]
    InvalidProofOfWork(Block),
    #[error("Invalid transaction: {0}")]
    InvalidTransaction(Transaction),
}

pub type HashArray = GenericArray<u8, typenum::U32>;

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize, Hash, Eq, Copy)]
pub struct Hash(pub HashArray);

impl Hash {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl From<HashArray> for Hash {
    fn from(array: HashArray) -> Self {
        Self(array)
    }
}

impl From<&[u8]> for Hash {
    fn from(slice: &[u8]) -> Self {
        Hash(*HashArray::from_slice(slice))
    }
}

impl From<&sha2::digest::Output<Sha256>> for Hash {
    fn from(hash: &sha2::digest::Output<Sha256>) -> Self {
        Hash::from(hash.as_slice())
    }
}

impl std::fmt::Display for Hash {
    /// 将 Hash 以十六进制的形式输出(小端序)，Lowercase hexadecimal encoding，即小端16进制编码
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 先遍历到的是低地址（即低位，小端序），所以要反转，这样输出的时候才是高位在前
        for byte in self.0.iter().rev() {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
// 所有字段都认为是小端序
pub struct Block {
    timestamp: i64,
    transactions: Vec<Transaction>,
    prev_hash: Hash,
    hash: Hash,
    nonce: BigUint,
}

impl Block {
    pub fn new(transactions: Vec<Transaction>, prev_hash: Hash) -> Result<Self, BlockError> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let mut block = Self {
            timestamp,
            transactions,
            prev_hash,
            hash: Hash::default(),
            nonce: BigUint::default(),
        };
        let (nonce, hash) = ProofOfWork::new(&block).run();
        block.nonce = nonce;
        block.hash = hash;

        Ok(block)
    }

    pub fn genesis(coinbase_tx: Transaction) -> Result<Self, BlockError> {
        Self::new(vec![coinbase_tx], Hash::default())
    }

    pub fn hash(&self) -> &Hash {
        &self.hash
    }

    pub fn prev_hash(&self) -> &Hash {
        &self.prev_hash
    }

    #[allow(dead_code)]
    pub fn timestamp(&self) -> i64 {
        self.timestamp
    }

    #[allow(dead_code)]
    pub fn transactions(&self) -> &Vec<Transaction> {
        &self.transactions
    }

    pub fn transactions_hash(&self) -> Hash {
        Hash::from(&Sha256::digest(
            bincode::serialize(&self.transactions).expect("Failed to serialize transactions"),
        ))
    }

    #[allow(dead_code)]
    pub fn nonce(&self) -> &BigUint {
        &self.nonce
    }
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "============ Block {} ============", self.hash)?;
        writeln!(f, "Prev. block: {}", self.prev_hash)?;
        writeln!(f, "PoW: {}", ProofOfWork::new(self).validate())?;
        for tx in &self.transactions {
            writeln!(f, "{}", tx)?;
        }
        writeln!(f)
    }
}
