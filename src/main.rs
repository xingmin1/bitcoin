use anyhow::Result;
use blockchain::Blockchain;
use log::{debug, info, warn};
use network::{Message, MessageData, Node};
use secp256k1::rand::{self, Rng};
use std::sync::{Arc, Mutex};
use utxo_set::UtxoSet;

mod block;
mod blockchain;
mod cli;
mod merkle_tree;
mod network;
mod proof_of_work;
mod transaction;
mod utxo_set;
mod wallet;

fn main() -> Result<()> {
    init_logger();
    info!("比特币节点模拟程序启动");

    let nodes = Node::create_nodes(2);
    info!("创建了 {} 个节点", nodes.len());

    let mut tasks = Vec::new();
    let final_nodes = Arc::new(Mutex::new(Vec::new()));

    for node in nodes {
        let final_nodes = Arc::clone(&final_nodes);
        tasks.push(std::thread::spawn(move || -> Result<()> {
            let final_node = task(node)?;
            final_nodes.lock().unwrap().push(final_node);
            Ok(())
        }));
    }

    for task in tasks {
        task.join().unwrap().unwrap();
    }

    // 打印最终状态
    let final_nodes = final_nodes.lock().unwrap();
    warn!("=== 最终区块链状态 ===");
    for node in final_nodes.iter() {
        if let Some(utxo_set) = &node.utxo_set {
            let blockchain = &utxo_set.blockchain;
            warn!("节点 {}: ", node.id);
            warn!("  区块链长度: {}", blockchain.length);
            warn!("  最新区块哈希: {}", blockchain.tip);
            warn!("  区块链为 {}", blockchain);
            warn!("  账户地址: {}", node.wallets.get_addresses()[0]);
        }
    }
    warn!("所有节点任务完成");

    Ok(())
}

fn init_logger() {
    // The `Env` lets us tweak what the environment
    // variables to read are and what the default
    // value is if they're missing
    let env = env_logger::Env::default()
        .filter_or("LOG_LEVEL", "warn")
        .write_style_or("LOG_STYLE", "always");

    env_logger::init_from_env(env);
}

fn task(mut node: Node) -> Result<Node> {
    info!("节点 {} 开始运行", node.id);
    node.send_address_to_all();

    let sleep_time = rand::thread_rng().gen_range(0..=10000);
    std::thread::sleep(std::time::Duration::from_millis(sleep_time));

    for round in 0..2 {
        debug!("节点 {} 开始第 {} 轮任务", node.id, round + 1);
        wait_for_message(&mut node);

        if node.utxo_set.is_none() {
            info!("节点 {} 创建新的区块链", node.id);
            let (blockchain, genesis) = Blockchain::new(
                node.wallets.get_addresses()[0].clone(),
                &Node::path_prefix(node.id),
            );
            let utxo_set = UtxoSet::new(blockchain);
            utxo_set.reindex(&Node::path_prefix(node.id));
            node.utxo_set = Some(utxo_set);
            if let Some(genesis) = genesis {
                debug!("节点 {} 发送创世区块给所有节点", node.id);
                node.send_message_to_all(Message::new(node.id, MessageData::Block { block_chain_length: 1, block: genesis }));
            }
        }

        while node.addresses.is_empty() {
            wait_for_message(&mut node);
        }

        // let send_count = rand::thread_rng().gen_range(1..=10);

        for _ in 0..(3-round) {
            debug!("节点 {} 开始转账", node.id);
            let to_ids = node.addresses.keys().cloned().collect::<Vec<_>>();
            let to_id = to_ids[rand::thread_rng().gen_range(0..to_ids.len())];
            let amount = rand::thread_rng().gen_range(1..=3);
            let balance = node.utxo_set.as_ref().unwrap().get_balance(node.wallets.get_addresses()[0], &Node::path_prefix(node.id));
            if balance < amount {
                continue;
            }
            debug!("节点 {} 向节点 {} 转账 {} 个币", node.id, to_id, amount);
            node.send(to_id, amount);
        }

        // 设置等待超时时间为100ms
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(100);

        while node.transaction_cache.len() < 2 && start_time.elapsed() < timeout {
            debug!(
                "节点 {} 等待交易，当前缓存交易数: {}",
                node.id,
                node.transaction_cache.len()
            );
            wait_for_message(&mut node);
        }

        if node.transaction_cache.is_empty() {
            continue;
        }
        info!("节点 {} 开始挖矿", node.id);
        node.mine();
        info!("节点 {} 完成挖矿", node.id);
    }

    info!("节点 {} 完成所有任务", node.id);
    wait_for_handle(&mut node);
    Ok(node)
}

/// 在一段时间内等待消息
fn wait_for_message(node: &mut Node) {
    // 设置等待时间为[0, 10]ms
    let duration = std::time::Duration::from_millis(rand::thread_rng().gen_range(0..=10));
    let start_time = std::time::Instant::now();

    // 在指定时间内循环接收消息
    while start_time.elapsed() < duration {
        // 尝试接收消息，设置超时时间为剩余时间
        if let Ok(message) = node
            .recv_channel
            .recv_timeout(duration.saturating_sub(start_time.elapsed()))
        {
            debug!("节点 {} 收到来自节点 {} 的消息", node.id, message.from_id);
            // 处理收到的消息
            let msg_id = message.from_id;
            message.handle(node, msg_id);
        }
    }
}

fn wait_for_handle(node: &mut Node) {
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(1000);
    while start_time.elapsed() < timeout {
        if let Ok(message) = node.recv_channel.recv_timeout(timeout.saturating_sub(start_time.elapsed())) {
            debug!("节点 {} 收到来自节点 {} 的消息", node.id, message.from_id);
            let msg_id = message.from_id;
            message.handle(node, msg_id);
        }
    }
}
