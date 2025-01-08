use std::collections::HashMap;

use crate::{
    block::{Block, BlockError, Hash},
    proof_of_work::ProofOfWork,
    transaction::{Transaction, TxOutputs},
};
use log::{debug, trace, warn};
use rocksdb::DB;
use thiserror::Error;

const DB_PATH: &str = "blockchain.db";

pub enum DbKey<'a> {
    Tip,
    Length,
    Block(&'a Hash),
}

impl<'a> AsRef<[u8]> for DbKey<'a> {
    fn as_ref(&self) -> &[u8] {
        match self {
            DbKey::Tip => b"tip",
            DbKey::Length => b"length",
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
    pub tip: Hash,
    pub length: u64,
}

impl Blockchain {
    pub fn new(genesis_address: String, path_prefix: &str) -> (Self, Option<Block>) {
        let db = DB::open_default(format!("{}/{}", path_prefix, DB_PATH)).unwrap();

        if let Ok(Some(tip)) = db.get(DbKey::Tip) {
            let length = db.get(DbKey::Length).unwrap().unwrap();
            (
                Self {
                    db,
                    tip: Hash::from(tip.as_slice()),
                    length: bincode::deserialize(&length).unwrap(),
                },
                None,
            )
        } else {
            debug!("create genesis block");

            let genesis_tx = Transaction::new_coinbase_tx(genesis_address, "".to_string());
            let genesis = Block::genesis(genesis_tx).unwrap();
            let tip = genesis.hash();
            db.put(DbKey::Block(tip), bincode::serialize(&genesis).unwrap())
                .unwrap();
            db.put(DbKey::Tip, tip.as_bytes()).unwrap();
            db.put(DbKey::Length, bincode::serialize(&1).unwrap())
                .unwrap();
            (
                Self {
                    db,
                    tip: *tip,
                    length: 1,
                },
                Some(genesis),
            )
        }
    }

    pub fn create_with_blocks(blocks: Vec<Block>, path_prefix: &str) -> Self {
        let db = DB::open_default(format!("{}/{}", path_prefix, DB_PATH)).unwrap();
        let tip = *blocks.first().unwrap().hash();
        db.put(DbKey::Tip, tip.as_bytes()).unwrap();
        db.put(
            DbKey::Length,
            bincode::serialize(&(blocks.len() as u64)).unwrap(),
        )
        .unwrap();
        let length = blocks.len() as u64;
        for block in blocks {
            db.put(
                DbKey::Block(block.hash()),
                bincode::serialize(&block).unwrap(),
            )
            .unwrap();
        }
        Self { db, tip, length }
    }

    /// 更新区块链，传入的区块集合必须合法
    ///
    /// 更新规则:
    /// 1. 如果传入的区块集合为空，则不更新
    /// 2. 如果传入的区块集合不合法，则不更新
    /// 3. 如果传入的区块集合的最后一个区块的prev_hash不等于当前区块链的tip，则视为新区块链，替换当前区块链
    /// 4. 如果传入的区块集合的最后一个区块的prev_hash等于当前区块链的tip，则视为在当前区块链基础上追加新区块
    pub fn update(&mut self, blocks: Vec<Block>, path_prefix: &str) {
        if blocks.is_empty() {
            return;
        }
        if Self::verify_blocks(&blocks).is_err() {
            return;
        }

        if blocks.last().unwrap().prev_hash != self.tip {
            assert_eq!(
                blocks.last().unwrap().prev_hash,
                Hash::default(),
                "区块链的创世纪区块的prev_hash必须为0"
            );
            assert!(
                blocks.len() > self.length as usize,
                "要替换的区块链长度必须大于当前区块链长度"
            );

            // std::fs::remove_dir_all(format!("{}/{}", path_prefix, DB_PATH)).unwrap();
            // self.db = DB::open_default(format!("{}/{}", path_prefix, DB_PATH)).unwrap();
            self.tip = Hash::default();
            self.length = 0;
        }

        let new_tip = *blocks.first().unwrap().hash();
        let new_length = self.length + blocks.len() as u64;
        for block in blocks {
            self.db
                .put(
                    DbKey::Block(block.hash()),
                    bincode::serialize(&block).unwrap(),
                )
                .unwrap();
        }
        self.db.put(DbKey::Tip, new_tip.as_bytes()).unwrap();
        self.db
            .put(DbKey::Length, bincode::serialize(&new_length).unwrap())
            .unwrap();
        self.tip = new_tip;
        self.length = new_length;
    }

    pub fn mine_block(&mut self, transactions: Vec<Transaction>) -> Result<Block, BlockchainError> {
        for tx in &transactions {
            if !self.verify_transaction(tx) {
                warn!("mine_block: 交易验证失败，交易id: {:?}", tx.id);
                return Err(BlockchainError::BlockError(BlockError::InvalidTransaction(
                    tx.clone(),
                )));
            }
        }
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
        self.length += 1;
        self.db
            .put(DbKey::Length, bincode::serialize(&self.length).unwrap())
            .unwrap();
        self.tip = *new_block.hash();
        Ok(new_block)
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
    /// 验证区块链是否合法
    pub fn verify_chain(&self) -> Result<(), BlockchainError> {
        let blocks: Vec<Block> = self.iter().collect();
        Self::verify_blocks(&blocks)
    }

    /// 验证区块序列是否合法
    ///
    /// 验证规则:
    /// 1. 每个区块的prev_hash必须等于前一个区块的hash
    /// 2. 每个区块的工作量证明必须有效
    pub fn verify_blocks(blocks: &[Block]) -> Result<(), BlockchainError> {
        // 添加一个默认区块用于验证创世区块
        let blocks_with_default: Vec<Block> = blocks
            .iter()
            .cloned()
            .chain(std::iter::once(Block::default()))
            .collect();

        // 先遍历到的是最新的区块
        for window in blocks_with_default.windows(2) {
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

    pub fn find_utxo(&self) -> HashMap<Hash, TxOutputs> {
        let mut utxo = HashMap::new();
        let mut spent_txos = HashMap::new();

        let blocks = self.iter().collect::<Vec<_>>();
        let transactions = blocks.iter().flat_map(|block| block.transactions());
        // 遍历所有交易，找到已经花费的输出
        transactions
            .clone()
            // 过滤掉 coinbase 交易
            .filter(|tx| !tx.is_coinbase())
            // 遍历所有输入
            .flat_map(|tx| &tx.vin)
            // 将address已经花费的输出加入spent_TXOs
            .for_each(|vin| {
                spent_txos
                    .entry(*vin.txid.as_ref().unwrap())
                    .or_insert_with(Vec::new)
                    .push(vin.vout.unwrap());
            });
        trace!("spent_txos: {:?}", spent_txos);
        // 遍历所有交易，找到未花费的输出
        transactions.for_each(|tx| {
            tx.vout
                .iter()
                .enumerate()
                .inspect(|(out_id, vout)| {
                    trace!("out_id: {}, vout: {:?}", out_id, vout);
                })
                // 过滤掉已经花费的输出
                .filter(|(out_id, _)| {
                    spent_txos.get(&tx.id).map_or(true, |spent_outputs| {
                        !spent_outputs.contains(&(*out_id as i32))
                    })
                })
                // 将未花费的输出加入utxo
                .for_each(|(out_id, vout)| {
                    trace!("out_id: {}, vout: {:?}", out_id, vout);
                    utxo.entry(tx.id)
                        .or_insert_with(TxOutputs::default)
                        .outputs
                        .push(vout.clone());
                    utxo.entry(tx.id)
                        .or_insert_with(TxOutputs::default)
                        .out_idxs
                        .push(out_id as i32);
                });
        });

        utxo
    }

    pub fn find_transaction(&self, id: &Hash) -> Option<Transaction> {
        self.iter()
            // TODO: 优化,在blockchain初始化时就将所以区块加载到内存中，防止以后每次都要从磁盘中读取
            .flat_map(|block| block.transactions().to_vec())
            .find(|tx| tx.id == *id)
    }

    pub fn sign_transaction(&self, tx: &mut Transaction, priv_key: secp256k1::SecretKey) {
        let prev_txs = tx
            .vin
            .iter()
            .map(|vin| {
                (
                    vin.txid.unwrap(),
                    self.find_transaction(&vin.txid.unwrap()).unwrap(),
                )
            })
            .collect::<HashMap<_, _>>();
        tx.sign(priv_key, &prev_txs);
    }

    pub fn verify_transaction(&self, tx: &Transaction) -> bool {
        if tx.is_coinbase() {
            return true;
        }

        let prev_txs = tx
            .vin
            .iter()
            .map(|vin| {
                let txid = vin.txid.unwrap();
                self.find_transaction(&txid).map(|prev_tx| (txid, prev_tx))
            })
            .collect::<Vec<_>>();

        // 如果有任何一个输入交易找不到，则返回false
        if prev_txs.iter().any(|tx| tx.is_none()) {
            return false;
        }

        let prev_txs = prev_txs
            .into_iter()
            .flatten()
            .collect::<HashMap<_, _>>();

        tx.verify(&prev_txs)
    }

    /// 将区块链中所有区块的哈希值转换为字符串表示
    /// 
    /// 返回一个字符串,每个区块的哈希值用 "\n -> " 连接,从最新的区块开始
    pub fn to_hashes_string(&self) -> String {
        self.iter()
            .map(|block| block.hash().to_string())
            .collect::<Vec<_>>()
            .join("\n -> ")
    }
}

impl std::fmt::Display for Blockchain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let blocks = self.iter().collect::<Vec<_>>();
        for block in blocks {
            writeln!(f, "{}", block)?;
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
