use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BlockError {
    #[error("Failed to get system time: {0}")]
    TimeError(#[from] std::time::SystemTimeError),
    #[error("Failed to convert data to string: {0}")]
    DataEncodingError(#[from] std::string::FromUtf8Error),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hash(Vec<u8>);

impl Hash {
    pub fn new(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        Self(hasher.finalize().to_vec())
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Default for Hash {
    fn default() -> Self {
        Self(vec![0; 32])
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in &self.0 {
            write!(f, "{:02x}", byte)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct Block {
    timestamp: i64,
    data: Vec<u8>,
    prev_hash: Hash,
    hash: Hash,
}

impl Block {
    pub fn new(data: Vec<u8>, prev_hash: Hash) -> Result<Self, BlockError> {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

        let mut block = Self {
            timestamp,
            data,
            prev_hash,
            hash: Hash::default(),
        };

        block.generate_hash();
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

    fn generate_hash(&mut self) {
        let data = [
            &self.timestamp.to_be_bytes(),
            self.data.as_slice(),
            self.prev_hash.as_bytes(),
        ]
        .concat();
        self.hash = Hash::new(&data);
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
