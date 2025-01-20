use log::{debug, error, info};

use crate::{
    block::{Block, Hash},
    blockchain::Blockchain,
    mem_pool::MemPool,
    transaction::{Transaction, TxOutputs},
    utxo_set::UtxoSet,
    wallet::Wallets,
    CliMessage,
};
use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver, Sender},
};

use super::message::{Message, MessageData};

pub struct Node {
    pub id: usize,
    pub utxo_set: Option<UtxoSet>,
    pub wallets: Wallets,
    pub mem_pool: Option<MemPool>,
    pub send_channels: Vec<Sender<Message>>,
    pub recv_channel: Receiver<Message>,
    pub cli_recv_channel: Receiver<CliMessage>,
    pub addresses: HashMap<usize, String>,
}

impl Node {
    pub fn new(
        id: usize,
        send_channels: Vec<Sender<Message>>,
        recv_channel: Receiver<Message>,
        cli_recv_channel: Receiver<CliMessage>,
    ) -> Self {
        let mut wallets = Wallets::new(&Node::path_prefix(id));
        if wallets.wallets.is_empty() {
            wallets.create_wallet();
            wallets.save_to_file(&Node::path_prefix(id));
        }

        Self {
            id,
            utxo_set: None,
            wallets,
            mem_pool: Some(MemPool::new()),
            send_channels,
            recv_channel,
            cli_recv_channel,
            addresses: HashMap::new(),
        }
    }

    pub fn blockchain(&mut self) -> Option<&mut Blockchain> {
        self.utxo_set
            .as_mut()
            .map(|utxo_set| &mut utxo_set.blockchain)
    }

    pub fn update_or_create_blockchain(&mut self, blocks: Vec<Block>) {
        let path = Node::path_prefix(self.id);
        match &mut self.utxo_set {
            None => {
                // 如果 UTXO 集合不存在，创建新的区块链和 UTXO 集合
                info!(target: "chain", "节点 {} 创建区块链，区块数量：{}", self.id, blocks.len());
                let blockchain = Blockchain::create_with_blocks(blocks, &path);
                self.utxo_set = Some(UtxoSet::new(blockchain));
                self.utxo_set.as_mut().unwrap().reindex(&path);
                self.mem_pool
                    .as_mut()
                    .unwrap()
                    .update_utxo_set(self.utxo_set.as_mut().unwrap().get_utxo_set(&path));
            }
            Some(utxo_set) => {
                // 如果 UTXO 集合存在，更新区块链
                debug!(target: "chain", "节点 {} 更新区块链，区块数量：{}", self.id, blocks.len());
                utxo_set.blockchain.update(blocks);
                self.mem_pool
                    .as_mut()
                    .unwrap()
                    .update_utxo_set(self.utxo_set.as_mut().unwrap().get_utxo_set(&path));
                self.mem_pool.as_mut().unwrap().transactions.retain(|tx| {
                    self.utxo_set
                        .as_mut()
                        .unwrap()
                        .blockchain
                        .find_transaction(&tx.id)
                        .is_some()
                });
            }
        }
        self.utxo_set.as_mut().unwrap().reindex(&path);
        self.mem_pool.as_mut().unwrap().clean_invalid_transaction(
            self.utxo_set
                .as_mut()
                .unwrap()
                .get_utxo_set(&Node::path_prefix(self.id)),
        );
    }

    pub fn send_message_to_all(&self, message: Message) {
        self.send_channels.iter().for_each(|channel| {
            let _ = channel.send(message.clone());
        });
    }

    pub fn path_prefix(node_id: usize) -> String {
        format!("data/node_{}", node_id)
    }

    pub fn clean_data(node_count: usize) {
        // 清理创建data目录
        (0..node_count).for_each(|i| {
            let data_dir = Node::path_prefix(i);
            let _ = std::fs::remove_dir_all(&data_dir);
            std::fs::create_dir_all(&data_dir).unwrap();
        });
    }

    /// 创建指定数量的节点
    pub fn create_nodes(node_count: usize) -> (Vec<Self>, Vec<Sender<CliMessage>>) {
        // 创建通信通道
        let (send_channels, recv_channels): (Vec<_>, Vec<_>) =
            (0..node_count).map(|_| channel::<Message>()).unzip();
        let (cli_send_channels, cli_recv_channels): (Vec<_>, Vec<_>) =
            (0..node_count).map(|_| channel::<CliMessage>()).unzip();

        // 创建节点
        let nodes = recv_channels
            .into_iter()
            .zip(cli_recv_channels)
            .enumerate()
            .map(|(i, (recv, cli_recv))| Self::new(i, send_channels.clone(), recv, cli_recv))
            .collect();
        (nodes, cli_send_channels)
    }

    /// 转账
    pub fn send(&mut self, to_id: usize, amount: u32) {
        let address = self.addresses.get(&to_id).unwrap();
        let (tx, given_utxo_set) = Transaction::new_utxo_transaction(
            self.wallets.get_addresses()[0].clone(),
            address.clone(),
            amount,
            self.utxo_set.as_ref().unwrap(),
            &Node::path_prefix(self.id),
            Some(self.mem_pool.as_ref().unwrap().utxo_set.clone()),
        );
        error!(
            "send tx from: {}, to: {}, amount: {}, inputs: {:?}",
            self.wallets.get_addresses()[0],
            address,
            amount,
            tx.vin
        );
        debug!(target: "chain", "send tx verify: {}", self.utxo_set.as_ref().unwrap().blockchain.verify_transaction(&tx));
        self.mem_pool.as_mut().unwrap().push(tx.clone());
        self.mem_pool.as_mut().unwrap().utxo_set = given_utxo_set.unwrap();
        debug!(target: "chain", "Transaction Cache: {}", self.mem_pool.as_ref().unwrap().len());
        self.send_message_to_all(Message::new(self.id, MessageData::Transaction(tx)));
    }

    /// 挖矿
    pub fn mine(&mut self) {
        let coinbase_tx = Transaction::new_coinbase_tx(
            self.wallets.get_addresses()[0].clone(),
            // format!("Reward to '{}'", self.id),
            "".to_string(),
        );
        let mut transactions = vec![coinbase_tx];
        transactions.extend(self.mem_pool.as_ref().unwrap().transactions.iter().cloned());
        let block = self
            .utxo_set
            .as_mut()
            .unwrap()
            .blockchain
            .mine_block(transactions)
            .unwrap();
        self.mem_pool.as_mut().unwrap().clear();
        self.utxo_set
            .as_mut()
            .unwrap()
            .reindex(&Node::path_prefix(self.id));
        self.mem_pool.as_mut().unwrap().utxo_set = self
            .utxo_set
            .as_mut()
            .unwrap()
            .get_utxo_set(&Node::path_prefix(self.id));
        let length = self.utxo_set.as_mut().unwrap().blockchain.length;
        let block_chain: Vec<Block> = self.utxo_set.as_mut().unwrap().blockchain.iter().collect();
        info!(
            target: "chain",
            "节点 {} 挖矿成功，区块数量：{}，block_chain长度：{}",
            self.id,
            length,
            block_chain.len()
        );
        self.send_message_to_all(Message::new(
            self.id,
            MessageData::BlockChain {
                block_chain_length: length,
                block_chain,
            },
        ));
    }

    pub fn send_address_to_all(&self) {
        self.send_message_to_all(Message::new(
            self.id,
            MessageData::Address(self.wallets.get_addresses()[0].clone()),
        ));
    }

    /// 整理交易缓存，使交易缓存中的交易符合区块链中的交易
    pub fn clean_invalid_transaction(&mut self) {
        let before_len = self.mem_pool.as_ref().unwrap().len();
        self.mem_pool.as_mut().unwrap().transactions.retain(|tx| {
            let exists = self
                .utxo_set
                .as_mut()
                .unwrap()
                .blockchain
                .find_transaction(&tx.id)
                .is_some();
            if exists {
                info!(target: "chain", "节点 {} 清理已存在的交易 {}", self.id, tx.id);
            }
            !exists
        });
        self.mem_pool.as_mut().unwrap().clean_invalid_transaction(
            self.utxo_set
                .as_mut()
                .unwrap()
                .get_utxo_set(&Node::path_prefix(self.id)),
        );
        if before_len != self.mem_pool.as_ref().unwrap().len() {
            info!(
                target: "chain",
                "节点 {} 清理交易缓存，从 {} 个减少到 {} 个",
                self.id,
                before_len,
                self.mem_pool.as_ref().unwrap().len()
            );
        }
    }
}
