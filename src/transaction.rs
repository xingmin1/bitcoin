use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Display;

use crate::{block::Hash, blockchain::Blockchain};

const COINBASE_AMOUNT: u32 = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: u32,
    pub script_pub_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub txid: Option<Hash>,
    pub vout: Option<i32>,
    pub script_sig: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Hash,
    pub vin: Vec<TxInput>,
    pub vout: Vec<TxOutput>,
}

impl Transaction {
    fn set_id(&mut self) {
        let encoded = bincode::serialize(&self).unwrap();
        self.id = Hash::from(&Sha256::digest(&encoded));
    }

    pub fn new_coinbase_tx(to: String, mut data: String) -> Self {
        if data.is_empty() {
            data.push_str(&format!("Reward to '{}'", to));
        }

        let txin = TxInput {
            txid: None,
            vout: None,
            script_sig: data,
        };
        let txout = TxOutput {
            value: COINBASE_AMOUNT,
            script_pub_key: to,
        };
        let mut tx = Self {
            id: Hash::default(),
            vin: vec![txin],
            vout: vec![txout],
        };
        tx.set_id();
        tx
    }

    pub fn is_coinbase(&self) -> bool {
        self.vin.len() == 1 && self.vin[0].txid.is_none() && self.vin[0].vout.is_none()
    }

    pub fn new_utxo_transaction(from: String, to: String, amount: u32, bc: &Blockchain) -> Self {
        let mut inputs = vec![];
        let mut outputs = vec![];

        let (acc, valid_outputs) = bc.find_spendable_outputs(&from, amount);
        if acc < amount {
            panic!("ERROR: Not enough funds");
        }

        for (txid, outs) in valid_outputs {
            for out in outs {
                let input = TxInput {
                    txid: Some(txid),
                    vout: Some(out),
                    script_sig: from.clone(),
                };
                inputs.push(input);
            }
        }

        outputs.push(TxOutput {
            value: amount,
            script_pub_key: to,
        });

        if acc > amount {
            outputs.push(TxOutput {
                value: acc - amount,
                script_pub_key: from,
            });
        }

        let mut tx = Self {
            id: Hash::default(),
            vin: inputs,
            vout: outputs,
        };
        tx.set_id();
        tx
    }
}

impl TxInput {
    pub fn can_unlock_output_with(&self, unlocking_data: &str) -> bool {
        self.script_sig == unlocking_data
    }
}

impl TxOutput {
    pub fn can_be_unlocked_with(&self, unlocking_data: &str) -> bool {
        self.script_pub_key == unlocking_data
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Transaction: {}", self.id)?;
        for (i, input) in self.vin.iter().enumerate() {
            writeln!(f, "Input #{}:", i)?;
            writeln!(
                f,
                "  TXID: {}",
                input.txid.as_ref().unwrap_or(&Hash::default())
            )?;
            writeln!(f, "  VOUT: {}", input.vout.unwrap_or(-1))?;
            writeln!(f, "  ScriptSig: {}", input.script_sig)?;
        }
        for (i, output) in self.vout.iter().enumerate() {
            writeln!(f, "Output #{}:", i)?;
            writeln!(f, "  Value: {}", output.value)?;
            writeln!(f, "  ScriptPubKey: {}", output.script_pub_key)?;
        }
        Ok(())
    }
}
