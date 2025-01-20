use log::{error, info};

use crate::block::Hash;
use crate::transaction::{Transaction, TxOutputs};
use std::collections::HashMap;

#[derive(Clone)]
pub struct MemPool {
    pub transactions: Vec<Transaction>,
    pub utxo_set: HashMap<Hash, TxOutputs>,
}

impl MemPool {
    pub fn new() -> Self {
        Self {
            transactions: vec![],
            utxo_set: HashMap::new(),
        }
    }

    pub fn push(&mut self, tx: Transaction) {
        self.transactions.push(tx);
    }

    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    pub fn clear(&mut self) {
        self.transactions.clear();
    }

    pub fn update_utxo_set(&mut self, utxo_set: HashMap<Hash, TxOutputs>) {
        self.utxo_set = utxo_set;
    }

    pub fn remove_utxo_set(utxo_set: &mut HashMap<Hash, TxOutputs>, txid: Hash, out_idxs: &[i32]) {
        utxo_set.get_mut(&txid).unwrap().remove_outputs(out_idxs);
        if utxo_set.get(&txid).unwrap().outputs.is_empty() {
            utxo_set.remove(&txid);
        }
    }

    pub fn clean_invalid_transaction(&mut self, mut utxo_set: HashMap<Hash, TxOutputs>) {
        self.transactions.retain(|tx| {
            for (txid, out) in tx.vin.iter().map(|input| (input.txid.unwrap(), input.vout.unwrap())) {
                if let Some(utxo) = utxo_set.get_mut(&txid) {
                    if !utxo.out_idxs.contains(&out) {
                        info!(target: "chain", "清理无效交易，找不到交易 {}", txid);
                        return false;
                    }
                    Self::remove_utxo_set(&mut utxo_set, txid, &[out]);
                } else {
                    info!(target: "chain", "清理无效交易，找不到交易 {}", txid);
                    return false;
                }
            }
            true
        });
        self.utxo_set = utxo_set;
    }
}

