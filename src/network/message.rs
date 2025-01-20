use log::{debug, error, info};

use crate::{
    block::Block,
    blockchain::Blockchain,
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
}

impl Message {
    pub fn new(from_id: usize, data: MessageData) -> Self {
        Self { from_id, data }
    }

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
                        info!(target: "chain",
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
                    debug!(target: "chain",
                        "节点 {} 收到更长的区块链，长度：{}，当前长度：{}",
                        node.id, block_chain_length, blockchain.length
                    );
                    node.update_or_create_blockchain(block_chain);
                }
            }

            MessageData::Transaction(transaction) => {
                info!(target: "chain", "节点 {} 收到交易消息: {}", node.id, transaction);
                // 铸币交易不应该被广播
                if transaction.is_coinbase() {
                    panic!("铸币交易不会被广播");
                }

                let blockchain = node.blockchain();
                if blockchain.is_none() {
                    debug!(target: "chain", "节点 {} 收到交易消息，但区块链不存在", node.id);
                    return;
                }

                // 检查交易是否已存在或无效
                if blockchain
                    .as_ref()
                    .unwrap()
                    .find_transaction(&transaction.id)
                    .is_some()
                    || !blockchain.unwrap().verify_transaction(&transaction)
                    || node.mem_pool.as_ref().unwrap().transactions.contains(&transaction)
                {
                    return;
                }

                // 将有效交易加入缓存
                node.mem_pool.as_mut().unwrap().push(transaction);
            }
            MessageData::Address(address) => {
                // 将地址加入到节点中
                node.addresses.insert(from_id, address);
            }
        }
    }
}
