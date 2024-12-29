use crate::{block::{Block, BlockError}, proof_of_work::ProofOfWork};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BlockchainError {
    #[error("Failed to create or process block: {0}")]
    BlockError(#[from] BlockError),

    #[error("Blockchain is empty")]
    EmptyChain,

    #[error("Invalid block sequence: hash mismatch")]
    InvalidBlockSequence,
}

#[derive(Debug)]
pub struct Blockchain {
    blocks: Vec<Block>,
}

impl Default for Blockchain {
    fn default() -> Self {
        Self::new()
    }
}

impl Blockchain {
    pub fn new() -> Self {
        let mut chain = Self { blocks: Vec::new() };
        if let Err(e) = chain.initialize() {
            // In a real application, you might want to handle this differently
            panic!("Failed to initialize blockchain: {}", e);
        }
        chain
    }

    fn initialize(&mut self) -> Result<(), BlockchainError> {
        let genesis = Block::genesis()?;
        self.blocks.push(genesis);
        Ok(())
    }

    pub fn add_block(&mut self, data: Vec<u8>) -> Result<(), BlockchainError> {
        let prev_block = self.last_block()?;
        let prev_hash = prev_block.hash().clone();
        let new_block = Block::new(data, prev_hash)?;

        // Optional: Validate block before adding
        self.validate_new_block(&new_block)?;

        self.blocks.push(new_block);
        Ok(())
    }

    pub fn last_block(&self) -> Result<&Block, BlockchainError> {
        self.blocks.last().ok_or(BlockchainError::EmptyChain)
    }

    #[allow(dead_code)]
    pub fn height(&self) -> usize {
        self.blocks.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    #[allow(dead_code)]
    pub fn get_block(&self, index: usize) -> Option<&Block> {
        self.blocks.get(index)
    }

    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter()
    }

    fn validate_new_block(&self, block: &Block) -> Result<(), BlockchainError> {
        let prev_block = self.last_block()?;

        if block.prev_hash() != prev_block.hash() {
            return Err(BlockchainError::InvalidBlockSequence);
        }

        Ok(())
    }

    // Optional: Add method to verify the entire chain
    #[allow(dead_code)]
    pub fn verify_chain(&self) -> Result<(), BlockchainError> {
        for window in self.blocks.windows(2) {
            let prev_block = &window[0];
            let current_block = &window[1];

            if current_block.prev_hash() != prev_block.hash() {
                return Err(BlockchainError::InvalidBlockSequence);
            }
        }
        Ok(())
    }
}

impl std::fmt::Display for Blockchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, block) in self.blocks.iter().enumerate() {
            writeln!(f, "Block #{}", i)?;
            writeln!(f, "{}", block)?;
            writeln!(f, "Pow: {}", ProofOfWork::new(block).validate())?;
            writeln!(f, "----------------")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_blockchain() {
        let blockchain = Blockchain::new();
        assert_eq!(blockchain.height(), 1);
        assert!(!blockchain.is_empty());
    }

    #[test]
    fn test_add_block() -> Result<(), BlockchainError> {
        let mut blockchain = Blockchain::new();
        blockchain.add_block(b"Test Block 1".to_vec())?;
        blockchain.add_block(b"Test Block 2".to_vec())?;

        assert_eq!(blockchain.height(), 3); // Genesis + 2 blocks
        Ok(())
    }

    #[test]
    fn test_chain_verification() -> Result<(), BlockchainError> {
        let mut blockchain = Blockchain::new();
        blockchain.add_block(b"Test Block".to_vec())?;

        assert!(blockchain.verify_chain().is_ok());
        Ok(())
    }

    #[test]
    fn test_blockchain_iteration() -> Result<(), BlockchainError> {
        let mut blockchain = Blockchain::new();
        blockchain.add_block(b"Test Block".to_vec())?;

        let blocks: Vec<&Block> = blockchain.iter().collect();
        assert_eq!(blocks.len(), 2); // Genesis + 1 block
        Ok(())
    }
}
