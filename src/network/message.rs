use log::warn;

use crate::{
    block::{Block, Hash},
    blockchain::Blockchain,
    proof_of_work::ProofOfWork,
    transaction::Transaction,
};

use super::node::Node;

#[derive(Debug, Clone)]
pub struct Message {
    pub from_id: usize,
    pub data: MessageData,
}

#[derive(Debug, Clone)]
pub enum MessageData {
    BlockChain {
        block_chain_length: u64,
        block_chain: Vec<Block>,
    },
    Transaction(Transaction),
    Address(String),
    GetTransaction(Hash),
}

impl Message {
    pub fn new(from_id: usize, data: MessageData) -> Self {
        Self { from_id, data }
    }
    // 处理消息{
    //     版本消息 -》 对比链长度，如果小于，则发送获取区块消息，阻塞等待区块消息，然后递归处理
    //     区块消息 -》 先验证区块，再验证区块中的交易，如果合法，则将该区块加入到区块缓存中，「
    //         如果该区块的前一个区块是本线程区块链的tip，则将该区块缓存加入到区块链中，并更新tip
    //         如果该区块的前一个区块不是本线程区块链的tip，则继续请求获取该区块的父区块，阻塞等待父区块消息，然后递归处理
    //     」
    //     交易消息 -》 验证交易，如果合法，则将该交易加入到交易缓存中

    //     获取区块消息 -》 在区块链中找到该区块，并发送该区块
    //     获取交易消息 -》 在交易缓存中找到该交易，并发送该交易
    //     获取版本消息 -》 将本线程区块链的版本号发送
    pub fn handle(self, node: &mut Node, from_id: usize) {
        match self.data {
            MessageData::BlockChain {
                block_chain_length,
                block_chain,
            } => {
                if Blockchain::verify_blocks(&block_chain).is_err() {
                    return;
                }

                let blockchain = match node.utxo_set.as_mut() {
                    Some(utxo_set) => &mut utxo_set.blockchain,
                    None => {
                        warn!(
                            "节点 {} 收到区块消息,但区块链不存在，接受到的区块数量：{}",
                            node.id,
                            block_chain.len()
                        );
                        // 如果区块链不存在,直接缓存区块
                        node.update_or_create_blockchain(block_chain);
                        return;
                    }
                };

                // 如果收到的合法区块链更长,则缓存区块
                if block_chain_length > blockchain.length {
                    node.update_or_create_blockchain(block_chain);
                }
            }

            MessageData::Transaction(transaction) => {
                // 铸币交易不应该被广播
                if transaction.is_coinbase() {
                    panic!("铸币交易不会被广播");
                }

                let blockchain = node.blockchain();

                // 检查交易是否已存在或无效
                if blockchain.is_none()
                    || blockchain
                        .as_ref()
                        .unwrap()
                        .find_transaction(&transaction.id)
                        .is_some()
                    || !blockchain.unwrap().verify_transaction(&transaction)
                    || node.transaction_cache.contains(&transaction)
                {
                    return;
                }

                // 将有效交易加入缓存
                node.transaction_cache.push(transaction);
            }
            MessageData::Address(address) => {
                // 将地址加入到节点中
                node.addresses.insert(from_id, address);
            }

            MessageData::GetTransaction(hash) => {
                // 查找并发送指定哈希值的交易
                if let Some(transaction) = node.transaction_cache.iter().find(|tx| tx.id == hash) {
                    node.send_message_to_one(
                        from_id,
                        Message::new(node.id, MessageData::Transaction(transaction.clone())),
                    );
                }
            }
        }
    }
}
