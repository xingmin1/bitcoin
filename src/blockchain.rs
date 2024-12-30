use crate::{
    block::{Block, BlockError, Hash},
    proof_of_work::ProofOfWork,
};
use rocksdb::DB;
use thiserror::Error;

const DB_PATH: &str = "blockchain.db";

pub enum DbKey<'a> {
    Tip,
    Block(&'a Hash),
}

impl<'a> AsRef<[u8]> for DbKey<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            DbKey::Tip => b"l",
            DbKey::Block(hash) => hash.as_bytes(),
        }
    }
}

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
    db: DB,
    tip: Hash,
}

impl Default for Blockchain {
    fn default() -> Self {
        Self::new()
    }
}

impl Blockchain {
    pub fn new() -> Self {
        let db = DB::open_default(DB_PATH).unwrap();

        if let Ok(Some(tip)) = db.get(DbKey::Tip) {
            Self {
                db,
                tip: bincode::deserialize(&tip).unwrap(),
            }
        } else {
            let genesis = Block::genesis().unwrap();
            let tip = genesis.hash();
            db.put(DbKey::Block(tip), bincode::serialize(&genesis).unwrap())
                .unwrap();
            db.put(DbKey::Tip, tip.as_bytes()).unwrap();
            Self {
                db,
                tip: tip.clone(),
            }
        }
    }

    pub fn add_block(&mut self, data: Vec<u8>) -> Result<(), BlockchainError> {
        let prev_hash =
            bincode::deserialize(&self.db.get_pinned(DbKey::Tip).unwrap().unwrap()).unwrap();
        let new_block = Block::new(data, prev_hash)?;

        // Optional: Validate block before adding
        self.validate_new_block(&new_block)?;

        self.db
            .put(
                DbKey::Block(new_block.hash()),
                bincode::serialize(&new_block).unwrap(),
            )
            .unwrap();
        self.db
            .put(DbKey::Tip, new_block.hash().as_bytes())
            .unwrap();
        self.tip = new_block.hash().clone();
        Ok(())
    }

    fn validate_new_block(&self, block: &Block) -> Result<(), BlockchainError> {
        let last_hasp = &self.tip;

        if block.prev_hash() != last_hasp {
            return Err(BlockchainError::InvalidBlockSequence);
        }

        if !ProofOfWork::new(block).validate() {
            return Err(BlockchainError::BlockError(BlockError::InvalidProofOfWork(
                block.clone(),
            )));
        }

        Ok(())
    }

    #[allow(dead_code)]
    pub fn verify_chain(&self) -> Result<(), BlockchainError> {
        let blocks: Vec<Block> = self
            .iter()
            // 为了验证创世区块，需要加入一个默认的区块
            .chain(std::iter::once(Block::default()))
            .collect();

        // 先遍历到的是最新的区块
        for window in blocks.windows(2) {
            let current_block = &window[0];
            let prev_block = &window[1];

            if current_block.prev_hash() != prev_block.hash() {
                return Err(BlockchainError::InvalidBlockSequence);
            }

            if !ProofOfWork::new(current_block).validate() {
                return Err(BlockchainError::BlockError(BlockError::InvalidProofOfWork(
                    current_block.clone(),
                )));
            }
        }
        Ok(())
    }

    /// 返回一个逆序迭代器( 最新的区块在前 )
    pub fn iter(&self) -> impl Iterator<Item = Block> + '_ {
        self.into_iter()
    }
}

impl std::fmt::Display for Blockchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chain = self.iter().collect::<Vec<_>>();
        for (i, block) in chain.iter().rev().enumerate() {
            writeln!(f, "Block #{}", i)?;
            writeln!(f, "{}", block)?;
            writeln!(f, "Pow: {}", ProofOfWork::new(block).validate())?;
            writeln!(f, "----------------")?;
        }
        Ok(())
    }
}

/// 逆序迭代器( 最新的区块在前 )
pub struct BlockchainIterator<'a> {
    db: &'a DB,
    current_hash: Hash,
}

impl<'a> Iterator for BlockchainIterator<'a> {
    type Item = Block;

    fn next(&mut self) -> Option<Self::Item> {
        let block = self
            .db
            .get_pinned(DbKey::Block(&self.current_hash))
            .unwrap()?;
        let block: Block = bincode::deserialize(&block).unwrap();
        self.current_hash = block.prev_hash().clone();
        Some(block)
    }
}

impl<'a> IntoIterator for &'a Blockchain {
    type Item = Block;
    type IntoIter = BlockchainIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        BlockchainIterator {
            db: &self.db,
            current_hash: self.tip.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_blockchain() {
        let blockchain = Blockchain::new();
        assert_eq!(blockchain.iter().fold(0, |init, _| { init + 1 }), 1);
    }

    #[test]
    fn test_add_block() -> Result<(), BlockchainError> {
        let mut blockchain = Blockchain::new();
        blockchain.add_block(b"Test Block 1".to_vec())?;
        blockchain.add_block(b"Test Block 2".to_vec())?;

        assert_eq!(blockchain.iter().fold(0, |init, _| { init + 1 }), 3);
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

        assert_eq!(blockchain.iter().fold(0, |init, _| { init + 1 }), 2);
        Ok(())
    }
}
