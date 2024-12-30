use generic_array::{typenum, GenericArray};
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

use crate::proof_of_work::ProofOfWork;

#[derive(Debug, Error)]
pub enum BlockError {
    #[error("Failed to get system time: {0}")]
    TimeError(#[from] std::time::SystemTimeError),
    #[error("Failed to convert data to string: {0}")]
    DataEncodingError(#[from] std::string::FromUtf8Error),
    #[error("Invalid proof of work: {0}")]
    InvalidProofOfWork(Block),
}

pub type HashArray = GenericArray<u8, typenum::U32>;

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
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

impl std::fmt::Display for Hash {
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
    data: Vec<u8>,
    prev_hash: Hash,
    hash: Hash,
    nonce: BigUint,
}

impl Block {
    pub fn new(data: Vec<u8>, prev_hash: Hash) -> Result<Self, BlockError> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let mut block = Self {
            timestamp,
            data,
            prev_hash,
            hash: Hash::default(),
            nonce: BigUint::default(),
        };
        let (nonce, hash) = ProofOfWork::new(&block).run();
        block.nonce = nonce;
        block.hash = hash;

        Ok(block)
    }

    pub fn genesis() -> Result<Self, BlockError> {
        Self::new(b"Genesis Block".to_vec(), Hash::default())
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
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[allow(dead_code)]
    pub fn nonce(&self) -> &BigUint {
        &self.nonce
    }
}

impl std::fmt::Display for Block {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Timestamp: {}", self.timestamp)?;
        writeln!(f, "Data: {}", String::from_utf8_lossy(&self.data))?;
        writeln!(f, "Previous Hash: {}", self.prev_hash)?;
        writeln!(f, "Hash: {}", self.hash)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_genesis_block() -> Result<(), BlockError> {
        let genesis = Block::genesis()?;
        assert_eq!(genesis.data(), b"Genesis Block");
        assert_eq!(genesis.prev_hash().as_bytes(), Hash::default().as_bytes());
        Ok(())
    }

    #[test]
    fn test_block_creation() -> Result<(), BlockError> {
        let genesis = Block::genesis()?;
        let data = b"Test Block".to_vec();
        let block = Block::new(data.clone(), genesis.hash().clone())?;

        assert_eq!(block.data(), data);
        assert_eq!(block.prev_hash().as_bytes(), genesis.hash().as_bytes());
        Ok(())
    }
}
