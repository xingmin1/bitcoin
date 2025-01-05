use generic_array::{typenum, GenericArray};
use log::trace;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fmt::Display};

use crate::{
    block::Hash, utxo_set::UtxoSet, wallet::{self, Wallets}
};

const COINBASE_AMOUNT: u32 = 50;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: u32,
    pub pub_key_hash: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TxOutputs {
    pub outputs: Vec<TxOutput>,
    /// 输出所在的索引
    pub out_idxs: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxInput {
    pub txid: Option<Hash>,
    pub vout: Option<i32>,
    pub signature: GenericArray<u8, typenum::U64>,
    pub pub_key: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Hash,
    pub vin: Vec<TxInput>,
    pub vout: Vec<TxOutput>,
}

impl Transaction {
    fn hash(&mut self) -> Hash {
        let mut tx_copy = self.clone();
        tx_copy.id = Hash::default();
        let encoded = bincode::serialize(&tx_copy).unwrap();
        Hash::from(&Sha256::digest(&encoded))
    }

    pub fn new_coinbase_tx(to_address: String, mut data: String) -> Self {
        if data.is_empty() {
            data.push_str(&format!("Reward to '{}'", to_address));
        }

        let txin = TxInput {
            txid: None,
            vout: None,
            signature: Default::default(),
            pub_key: data.into_bytes(),
        };

        let decoded = bs58::decode(to_address).into_vec().unwrap();
        let to_pub_key_hash = &decoded[1..decoded.len() - 4];
        trace!("new_coinbase_tx to_pub_key_hash: {:?}", to_pub_key_hash);
        let txout = TxOutput {
            value: COINBASE_AMOUNT,
            pub_key_hash: to_pub_key_hash.to_vec(),
        };
        let mut tx = Self {
            id: Hash::default(),
            vin: vec![txin],
            vout: vec![txout],
        };
        tx.id = tx.hash();
        tx
    }

    /// 对交易的每个输入分别进行签名，签名的数据是交易副本的哈希值
    /// 签名时，其他输入的签名和公钥字段为空，被签名的输入的公钥字段被设置为对应输出的公钥哈希，但签名字段为空
    /// 签名时，所有新的输出的公钥哈希和输出值都被包含。
    pub fn sign(
        &mut self,
        private_key: secp256k1::SecretKey,
        prev_txs: &HashMap<Hash, Transaction>,
    ) {
        if self.is_coinbase() {
            return;
        }

        let mut tx_copy = self.trimmed_copy();
        for (i, input) in self.vin.iter_mut().enumerate() {
            let prev_tx = prev_txs.get(&input.txid.unwrap()).unwrap();
            tx_copy.vin[i].signature = Default::default();
            tx_copy.vin[i].pub_key = prev_tx.vout[input.vout.unwrap() as usize]
                .pub_key_hash
                .clone();
            tx_copy.id = tx_copy.hash();
            tx_copy.vin[i].pub_key = vec![];

            let secp = secp256k1::Secp256k1::new();
            let msg = secp256k1::Message::from_digest(
                tx_copy
                    .id
                    .as_bytes()
                    .try_into()
                    .expect("Failed to convert to [u8; 32]"),
            );
            let sig = secp.sign_ecdsa(&msg, &private_key);
            input.signature = sig.serialize_compact().into();
        }
    }

    /// 验证交易的输入是否有效
    /// 验证时，需要对交易的每个输入进行验证，验证的数据是交易副本的哈希值
    /// 验证时，其他输入的签名和公钥字段为空，被验证的输入的公钥字段被设置为对应输出的公钥哈希，但签名字段为空
    /// 验证时，所有新的输出的公钥哈希和输出值都被包含。
    /// 与签名对称
    pub fn verify(&self, prev_txs: &HashMap<Hash, Transaction>) -> bool {
        if self.is_coinbase() {
            return true;
        }

        let mut tx_copy = self.trimmed_copy();
        let secp = secp256k1::Secp256k1::new();

        for (i, input) in self.vin.iter().enumerate() {
            let prev_tx = prev_txs.get(&input.txid.unwrap()).unwrap();
            tx_copy.vin[i].signature = Default::default();
            tx_copy.vin[i].pub_key = prev_tx.vout[input.vout.unwrap() as usize]
                .pub_key_hash
                .clone();
            tx_copy.id = tx_copy.hash();
            tx_copy.vin[i].pub_key = prev_tx.vout[input.vout.unwrap() as usize]
                .pub_key_hash
                .clone();

            let msg = secp256k1::Message::from_digest(
                tx_copy
                    .id
                    .as_bytes()
                    .try_into()
                    .expect("Failed to convert to [u8; 32]"),
            );
            let sig = secp256k1::ecdsa::Signature::from_compact(&input.signature).unwrap();
            let pub_key = secp256k1::PublicKey::from_slice(&input.pub_key).unwrap();
            if secp.verify_ecdsa(&msg, &sig, &pub_key).is_err() {
                return false;
            }
        }

        true
    }

    /// 返回修剪后的交易副本，其中输入的签名和公钥字段为空
    fn trimmed_copy(&self) -> Self {
        let mut inputs = vec![];
        let mut outputs = vec![];

        for input in &self.vin {
            inputs.push(TxInput {
                txid: input.txid,
                vout: input.vout,
                signature: Default::default(),
                pub_key: vec![],
            });
        }

        for output in &self.vout {
            outputs.push(TxOutput {
                value: output.value,
                pub_key_hash: output.pub_key_hash.clone(),
            });
        }

        Self {
            id: self.id,
            vin: inputs,
            vout: outputs,
        }
    }

    pub fn is_coinbase(&self) -> bool {
        self.vin.len() == 1 && self.vin[0].txid.is_none() && self.vin[0].vout.is_none()
    }

    pub fn new_utxo_transaction(from: String, to: String, amount: u32, utxo_set: &UtxoSet) -> Self {
        let mut inputs = vec![];
        let mut outputs = vec![];

        let wallets = Wallets::new();
        let wallet = wallets.get_wallet(&from).unwrap();
        let pub_key_hash = wallet::hash_pub_key(&wallet.public_key);
        trace!("new_utxo_transaction pub_key_hash: {:?}", pub_key_hash);

        let (acc, valid_outputs) = utxo_set.find_spendable_outputs(&pub_key_hash, amount);
        if acc < amount {
            panic!("ERROR: Not enough funds");
        }

        for (txid, outs) in valid_outputs {
            for out in outs {
                let input = TxInput {
                    txid: Some(txid),
                    vout: Some(out),
                    signature: Default::default(),
                    pub_key: wallet.public_key.clone(),
                };
                inputs.push(input);
            }
        }

        let decoded = bs58::decode(to).into_vec().unwrap();
        let to_pub_key_hash = &decoded[1..decoded.len() - 4];
        outputs.push(TxOutput {
            value: amount,
            pub_key_hash: to_pub_key_hash.to_vec(),
        });

        if acc > amount {
            outputs.push(TxOutput {
                value: acc - amount,
                pub_key_hash: pub_key_hash.clone(),
            });
        }

        let mut tx = Self {
            id: Hash::default(),
            vin: inputs,
            vout: outputs,
        };
        tx.id = tx.hash();
        utxo_set.blockchain.sign_transaction(&mut tx, wallet.private_key);
        tx
    }
}

impl TxInput {
    pub fn uses_key(&self, pub_key_hash: &[u8]) -> bool {
        pub_key_hash == wallet::hash_pub_key(&self.pub_key)
    }
}

impl TxOutput {
    pub fn is_locked_with_key(&self, pub_key_hash: &[u8]) -> bool {
        self.pub_key_hash == pub_key_hash
    }

    pub fn lock(&mut self, address: &[u8]) {
        // 比特币地址解码后的长度为 25 字节
        // 第一个字节是版本号，后面 20 字节是公钥哈希，最后 4 字节是校验和
        let decoded: [u8; 25] = bs58::decode(address).into_array_const_unwrap();
        self.pub_key_hash = decoded[1..21].to_vec();
    }
}

impl Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut lines = vec![format!("--- Transaction {}:", self.id)];

        for (i, input) in self.vin.iter().enumerate() {
            lines.push(format!("     Input {}:", i));
            lines.push(format!("       TXID:      {:?}", input.txid));
            lines.push(format!("       Out:       {:?}", input.vout));
            lines.push(format!("       Signature: {:?}", input.signature));
            lines.push(format!("       PubKey:    {:?}", input.pub_key));
        }

        for (i, output) in self.vout.iter().enumerate() {
            lines.push(format!("     Output {}:", i));
            lines.push(format!("       Value:  {:?}", output.value));
            lines.push(format!("       Script: {:?}", output.pub_key_hash));
        }

        write!(f, "{}", lines.join("\n"))
    }
}
