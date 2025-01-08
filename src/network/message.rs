use crate::{block::{Block, Hash}, proof_of_work::ProofOfWork, transaction::Transaction};

use super::node::Node;

#[derive(Debug, Clone)]
pub struct Message {
    pub from_address: String,
    pub data: MessageData,
}

#[derive(Debug, Clone)]
pub enum MessageData {
    Block{
        block_chain_length: u64,
        block: Block,
    },
    Transaction(Transaction),
    GetBlock(Hash),
    GetTransaction(Hash),
}

impl Message {
    pub fn new(from_address: String, data: MessageData) -> Self {
        Self { from_address, data }
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
    pub fn handle(self, node: &mut Node) {
        match self.data {
            MessageData::Block { block_chain_length, block } => {
                // 验证区块的PoW
                if !ProofOfWork::new(&block).validate() {
                    return;
                }

                // 如果区块链不存在或者收到的区块链更长，则缓存区块
                let should_cache = node.blockchain()
                    .map_or(true, |blockchain| block_chain_length > blockchain.length);
                if should_cache {
                    node.block_cache.push(block.clone());
                }

                // 如果是创世区块或者区块可以接在当前链上，则更新区块链
                let can_update = block.prev_hash == Hash::default() || 
                    node.blockchain().map_or(false, |blockchain| blockchain.tip == block.prev_hash);
                if can_update {
                    node.update_or_create_blockchain(node.block_cache.clone());
                    node.block_cache.clear();
                } else {
                    // 如果区块不能接在当前链上，则请求获取该区块的父区块，阻塞等待父区块消息，然后递归处理
                    todo!()
                }
            }
            
            MessageData::Transaction(transaction) => {
                if transaction.is_coinbase() {
                    panic!("铸币交易不会被广播");
                }

                // 如果交易已经存在于区块链中或者交易无效，则不处理
                if node.blockchain().unwrap().find_transaction(&transaction.id).is_some() || node.blockchain().unwrap().verify_transaction(&transaction) {
                    return;
                }

                // 如果交易已经存在于交易缓存中，则不处理
                if node.transaction_cache.contains(&transaction) {
                    return;
                }
                

                // 将交易加入到交易缓存中
                node.transaction_cache.push(transaction);
            }

            MessageData::GetBlock(hash) => {
                // 在区块链中找到该区块，并发送该区块
                let block = node.blockchain().unwrap().iter().find(|block| block.hash == hash).unwrap();
                
                todo!("发送区块")
            }

            MessageData::GetTransaction(hash) => {
                // 在交易缓存中找到该交易，并发送该交易
                let transaction = node.transaction_cache.iter().find(|transaction| transaction.id == hash).unwrap();
                todo!("发送交易")
            }
        }
    }
}

