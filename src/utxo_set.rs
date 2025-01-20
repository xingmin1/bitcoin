use std::collections::HashMap;

use log::{debug, error, info};
use rocksdb::DB;

use crate::{
    block::{Block, Hash},
    blockchain::Blockchain,
    transaction::{TxOutput, TxOutputs},
};

/// UTXO集合的数据库文件名
const UTXO_SET_DB_NAME: &str = "utxo_set.db";

/// UTXO集合，用于管理未花费的交易输出
///
/// 这个结构体维护了一个独立的数据库来存储所有未花费的交易输出（UTXO）。
/// 相比于在区块链中直接查找UTXO，这种方式能显著提高查询效率。
#[derive(Debug)]
pub struct UtxoSet {
    pub blockchain: Blockchain,
}

impl UtxoSet {
    /// 创建新的UTXO集合
    pub fn new(blockchain: Blockchain) -> Self {
        Self { blockchain }
    }

    /// 重建UTXO集合
    ///
    /// 扫描整个区块链，重新构建UTXO集合。这个操作会删除现有的UTXO数据库并重新创建。
    pub fn reindex(&self, path_prefix: &str) {
        let _ = std::fs::remove_dir_all(format!("{}/{}", path_prefix, UTXO_SET_DB_NAME));
        let db = DB::open_default(format!("{}/{}", path_prefix, UTXO_SET_DB_NAME)).unwrap();
        let utxo_map = self.blockchain.find_utxo();
        for (txid, outputs) in utxo_map {
            let serialized_outputs = bincode::serialize(&outputs).unwrap();
            db.put(txid.as_bytes(), serialized_outputs).unwrap();
        }
        db.flush().unwrap();
        debug!("reindex utxo set success");
    }

    /// 查找指定数额的可花费输出
    ///
    /// 遍历UTXO集合，累计输出直到达到指定金额。这个方法用于构建新的交易。
    ///
    /// # 返回值
    /// - 返回一个元组 (accumulated, unspent_outputs)
    ///   - accumulated: 累计找到的金额
    ///   - unspent_outputs: 交易ID到输出索引的映射
    pub fn find_spendable_outputs(
        &self,
        pub_key_hash: &[u8],
        amount: u32,
        path_prefix: &str,
        given_utxo_set: Option<HashMap<Hash, TxOutputs>>,
    ) -> (u32, HashMap<Hash, Vec<i32>>) {
        let utxo_set: HashMap<Hash, TxOutputs> = given_utxo_set.unwrap_or_else(|| {
            let db = DB::open_default(format!("{}/{}", path_prefix, UTXO_SET_DB_NAME)).unwrap();
            db.iterator(rocksdb::IteratorMode::Start).map(|item| {
                let (k, v) = item.unwrap();
                let txid = Hash::from(&*k);
                let outputs: TxOutputs = bincode::deserialize(&v).unwrap();
                (txid, outputs)
            }).collect()
        });
        let mut unspent_outputs = HashMap::new();
        let mut accumulated = 0;

        for (txid, outputs) in utxo_set {
            for (out_id, output) in outputs.outputs.iter().enumerate() {
                if output.is_locked_with_key(pub_key_hash) && accumulated < amount {
                    accumulated += output.value;
                    unspent_outputs
                        .entry(txid)
                        .or_insert_with(Vec::new)
                        .push(outputs.out_idxs[out_id]);
                }
            }
        }
        (accumulated, unspent_outputs)
    }

    /// 查找地址的所有未花费输出
    pub fn find_utxo(&self, pub_key_hash: &[u8], path_prefix: &str, given_utxo_set: Option<HashMap<Hash, TxOutputs>>) -> Vec<TxOutput> {
        let utxo_set: HashMap<Hash, TxOutputs> = given_utxo_set.unwrap_or_else(|| {
            let db = DB::open_default(format!("{}/{}", path_prefix, UTXO_SET_DB_NAME)).unwrap();
            db.iterator(rocksdb::IteratorMode::Start).map(|item| {
                let (k, v) = item.unwrap();
                let txid = Hash::from(&*k);
                let outputs: TxOutputs = bincode::deserialize(&v).unwrap();
                (txid, outputs)
            }).collect()
        });
        let mut utxos = Vec::new();
        for (_, outputs) in utxo_set {
            utxos.extend(
                outputs
                    .outputs
                    .iter()
                    .filter(|output| output.is_locked_with_key(pub_key_hash))
                    .cloned(),
            );
        }
        utxos
    }

    pub fn get_utxo_set(&self, path_prefix: &str) -> HashMap<Hash, TxOutputs> {
        let db = DB::open_default(format!("{}/{}", path_prefix, UTXO_SET_DB_NAME)).unwrap();
        db.iterator(rocksdb::IteratorMode::Start).map(|item| {
            let (k, v) = item.unwrap();
            let txid = Hash::from(&*k);
            let outputs: TxOutputs = bincode::deserialize(&v).unwrap();
            (txid, outputs)
        }).collect()
    }


    pub fn get_balance(&self, address: &str, path_prefix: &str, given_utxo_set: Option<HashMap<Hash, TxOutputs>>) -> u32 {
        let decoded = bs58::decode(address).into_vec().unwrap();
        let pub_key_hash = &decoded[1..decoded.len() - 4];
        let utxos = self.find_utxo(pub_key_hash, path_prefix, given_utxo_set);
        info!(target: "chain", "utxos: {:?}", utxos);
        utxos.iter().map(|output| output.value).sum()
    }

    /// 更新UTXO集合
    ///
    /// 处理新区块中的所有交易：
    /// 1. 删除已花费的输出
    /// 2. 添加新的未花费输出
    pub fn update(&self, block: &Block, path_prefix: &str) {
        let db = DB::open_default(format!("{}/{}", path_prefix, UTXO_SET_DB_NAME)).unwrap();

        // 处理交易输入，删除已花费的输出
        block
            .transactions()
            .iter()
            .filter(|tx| !tx.is_coinbase())
            .flat_map(|tx| tx.vin.iter())
            .for_each(|input| {
                let txid = input.txid.unwrap();
                let out_idx = input.vout.unwrap();
                debug!("txid: {}", txid);
                let mut outputs: TxOutputs =
                    bincode::deserialize(&db.get(txid.as_bytes()).expect("get utxo set db failed").unwrap()).unwrap();
                let out_idx = outputs.out_idxs.iter().position(|&idx| idx == out_idx).unwrap();
                outputs.outputs.remove(out_idx as usize);
                outputs.out_idxs.remove(out_idx as usize);
                if outputs.outputs.is_empty() {
                    db.delete(txid.as_bytes()).unwrap();
                } else {
                    db.put(txid.as_bytes(), bincode::serialize(&outputs).unwrap())
                        .unwrap();
                }
            });

        // 处理交易输出，添加新的UTXO
        block.transactions().iter().for_each(|tx| {
            let outputs = TxOutputs {
                out_idxs: (0..tx.vout.len()).map(|i| i as i32).collect(),
                outputs: tx.vout.clone(),
            };
            let serialized_outputs = bincode::serialize(&outputs).unwrap();
            db.put(tx.id.as_bytes(), serialized_outputs).unwrap();
        });
        db.flush().unwrap();
        debug!("update utxo set success");
    }
}

