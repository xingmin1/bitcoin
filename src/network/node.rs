use crate::{
    block::Block,
    blockchain::Blockchain,
    transaction::Transaction,
    utxo_set::UtxoSet,
    wallet::Wallets,
};
use std::sync::mpsc::{channel, Receiver, Sender};

use super::message::Message;

pub struct Node {
    pub id: usize,
    pub utxo_set: Option<UtxoSet>,
    pub wallets: Wallets,
    pub block_cache: Vec<Block>,
    pub transaction_cache: Vec<Transaction>,
    pub send_channels: Vec<Sender<Message>>,
    pub recv_channel: Receiver<Message>,
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
            transaction_cache: vec![],
            send_channels,
            recv_channel,
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
            .for_each(|channel| channel.send(message.clone()).unwrap());
    }

    pub fn send_message_to_one(&self, node_id: usize, message: Message) {
        let channel = &self.send_channels[node_id];
        channel.send(message).unwrap();
    }

    pub fn path_prefix(node_id: usize) -> String {
        format!("node_{}", node_id)
    }

    /// 创建指定数量的节点
    pub fn create_nodes(node_count: usize) -> Vec<Self> {
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
}
