use crate::{
    block::Block,
    blockchain::Blockchain,
    transaction::Transaction,
    utxo_set::UtxoSet,
    wallet::Wallets,
};
use std::{collections::HashMap, sync::mpsc::{channel, Receiver, Sender}};

use super::message::{Message, MessageData};

pub struct Node {
    pub id: usize,
    pub utxo_set: Option<UtxoSet>,
    pub wallets: Wallets,
    pub block_cache: Vec<Block>,
    pub block_cache_from_id: Option<usize>,
    pub transaction_cache: Vec<Transaction>,
    pub send_channels: Vec<Sender<Message>>,
    pub recv_channel: Receiver<Message>,
    pub addresses: HashMap<usize, String>,
}

impl Node {
    pub fn new(
        id: usize,
        send_channels: Vec<Sender<Message>>,
        recv_channel: Receiver<Message>,
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
            block_cache: vec![],
            block_cache_from_id: None,
            transaction_cache: vec![],
            send_channels,
            recv_channel,
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
                let blockchain = Blockchain::create_with_blocks(blocks, &path);
                self.utxo_set = Some(UtxoSet::new(blockchain));
            }
            Some(utxo_set) => {
                // 如果 UTXO 集合存在，更新区块链
                utxo_set.blockchain.update(blocks, &path);
            }
        }
    }

    pub fn send_message_to_all(&self, message: Message) {
        self.send_channels
            .iter()
            .for_each(|channel| { let _ = channel.send(message.clone()); });
    }

    pub fn send_message_to_one(&self, node_id: usize, message: Message) {
        let channel = &self.send_channels[node_id];
        let _ = channel.send(message);
    }

    pub fn path_prefix(node_id: usize) -> String {
        format!("data/node_{}", node_id)
    }

    /// 创建指定数量的节点
    pub fn create_nodes(node_count: usize) -> Vec<Self> {

        // 清理创建data目录
        (0..node_count).for_each(|i| {
            let data_dir = Node::path_prefix(i);
            let _ = std::fs::remove_dir_all(&data_dir);
            std::fs::create_dir_all(&data_dir).unwrap();
        });

        // 创建通信通道
        let (send_channels, recv_channels): (Vec<_>, Vec<_>) = (0..node_count)
            .map(|_| channel::<Message>())
            .unzip();

        // 创建节点
        recv_channels
            .into_iter()
            .enumerate()
            .map(|(i, recv)| Self::new(i, send_channels.clone(), recv))
            .collect()
    }

    /// 转账
    pub fn send(&mut self, to_id: usize, amount: u32) {
        let address = self.addresses.get(&to_id).unwrap();
        let tx = Transaction::new_utxo_transaction(self.wallets.get_addresses()[0].clone(), address.clone(), amount, self.utxo_set.as_ref().unwrap(), &Node::path_prefix(self.id));
        self.transaction_cache.push(tx.clone());
        self.send_message_to_all(Message::new(self.id, MessageData::Transaction(tx)));
    }

    /// 挖矿
    pub fn mine(&mut self) {
        let block = self.utxo_set.as_mut().unwrap().blockchain.mine_block(self.transaction_cache.clone()).unwrap();
        self.transaction_cache.clear();
        let length = self.utxo_set.as_mut().unwrap().blockchain.length;
        self.send_message_to_all(Message::new(self.id, MessageData::Block {
            block_chain_length: length,
            block: block.clone(),
        }));
    }

    pub fn send_address_to_all(&self) {
        self.send_message_to_all(Message::new(self.id, MessageData::Address(self.wallets.get_addresses()[0].clone())));
    }
}

