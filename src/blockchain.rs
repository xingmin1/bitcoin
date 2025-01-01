use std::collections::HashMap;

use crate::{
    block::{Block, BlockError, Hash},
    proof_of_work::ProofOfWork,
    transaction::{self, Transaction},
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

impl Blockchain {
    pub fn new(genesis_address: String) -> Self {
        let db = DB::open_default(DB_PATH).unwrap();

        if let Ok(Some(tip)) = db.get(DbKey::Tip) {
            Self {
                db,
                tip: bincode::deserialize(&tip).unwrap(),
            }
        } else {
            let genesis_tx = Transaction::new_coinbase_tx(genesis_address, "".to_string());
            let genesis = Block::genesis(genesis_tx).unwrap();
            let tip = genesis.hash();
            db.put(DbKey::Block(tip), bincode::serialize(&genesis).unwrap())
                .unwrap();
            db.put(DbKey::Tip, tip.as_bytes()).unwrap();
            Self {
                db,
                tip: *tip,
            }
        }
    }

    pub fn mine_block(&mut self, transactions: Vec<Transaction>) -> Result<(), BlockchainError> {
        let prev_hash =
            bincode::deserialize(&self.db.get_pinned(DbKey::Tip).unwrap().unwrap()).unwrap();
        let new_block = Block::new(transactions, prev_hash)?;

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
        self.tip = *new_block.hash();
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

    /// 这个方法对所有的未花费交易进行迭代，并对它的值进行累加。当累加值大于或等于amount时，它就会停止并返回累加值，同时返回的还有通过交易 ID 进行分组的输出索引
    pub fn find_spendable_outputs(
        &self,
        address: &str,
        amount: u32,
    ) -> (u32, HashMap<Hash, Vec<i32>>) {
        let mut unspent_outputs = HashMap::new();
        let mut accumulated = 0;

        'outer: for tx in self.find_unspent_transactions(address) {
            let txid = tx.id;
            for (out_id, out) in tx.vout.iter().enumerate() {
                if out.can_be_unlocked_with(address) && accumulated < amount {
                    accumulated += out.value;
                    unspent_outputs
                        .entry(txid)
                        .or_insert_with(Vec::new)
                        .push(out_id as i32);
                    if accumulated >= amount {
                        break 'outer;
                    }
                }
            }
        }

        (accumulated, unspent_outputs)
    }

    /// 返回一个address的未花费的交易列表
    pub fn find_unspent_transactions(&self, address: &str) -> Vec<Transaction> {
        #![allow(non_snake_case)]
        let mut unspent_TXs = vec![];
        let mut spent_TXOs = HashMap::new();

        let blocks = self.iter().collect::<Vec<_>>();
        let transactions = blocks.iter().flat_map(|block| block.transactions());

        // 遍历所有交易，找到已经花费的输出
        transactions
            .clone()
            // 过滤掉 coinbase 交易
            .filter(|tx| !tx.is_coinbase())
            // 遍历所有输入
            .flat_map(|tx| &tx.vin)
            // 过滤掉不是当前地址的输入
            .filter(|vin| vin.can_unlock_output_with(address))
            // 将address已经花费的输出加入spent_TXOs
            .for_each(|vin| {
                spent_TXOs
                    .entry(*vin.txid.as_ref().unwrap())
                    .or_insert_with(Vec::new)
                    .push(vin.vout.unwrap());
            });

        // 遍历所有交易，找到未花费的输出
        transactions.for_each(|tx| {
            tx.vout
                .iter()
                .enumerate()
                // 过滤掉已经花费的输出
                .filter(|(out_id, _)| {
                    spent_TXOs.get(&tx.id).map_or(true, |spent_outputs| {
                        !spent_outputs.contains(&(*out_id as i32))
                    })
                })
                // 过滤掉不是当前地址的输出
                .filter(|(_, vout)| vout.can_be_unlocked_with(address))
                // 将未花费的输出加入unspent_TXs
                .for_each(|_| {
                    unspent_TXs.push(tx.clone());
                });
        });

        unspent_TXs
    }

    pub fn find_utxo(&self, address: &str) -> Vec<transaction::TxOutput> {
        self.find_unspent_transactions(address)
            .into_iter()
            .flat_map(|tx| tx.vout)
            .filter(|vout| vout.can_be_unlocked_with(address))
            .collect()
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
        self.current_hash = *block.prev_hash();
        Some(block)
    }
}

impl<'a> IntoIterator for &'a Blockchain {
    type Item = Block;
    type IntoIter = BlockchainIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        BlockchainIterator {
            db: &self.db,
            current_hash: self.tip,
        }
    }
}
