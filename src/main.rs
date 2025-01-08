use anyhow::Result;
use blockchain::Blockchain;
use log::{debug, info, warn};
use network::{Message, MessageData, Node};
use secp256k1::rand::{self, Rng};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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

// 系统配置常量
const NODE_COUNT: usize = 4; // 节点数量
const MINING_ROUNDS: usize = 10; // 每个节点的挖矿轮数
const MIN_TRANSACTIONS_TO_MINE: usize = 2; // 开始挖矿所需的最小交易数
const INITIAL_WAIT_TIME: u64 = 100; // 节点启动时的最大等待时间(ms)
const MESSAGE_WAIT_TIME: u64 = 10; // 等待消息的最大时间(ms)
const TRANSACTION_WAIT_TIMEOUT: u64 = 10; // 等待交易的超时时间(ms)
const MESSAGE_HANDLE_TIMEOUT: u64 = 1000; // 消息处理的超时时间(ms)
const MIN_TRANSACTION_AMOUNT: u32 = 1; // 最小转账金额
const MAX_TRANSACTION_AMOUNT: u32 = 3; // 最大转账金额

// 完成任务的节点计数器
static FINISHED_COUNT: AtomicUsize = AtomicUsize::new(0);

fn main() -> Result<()> {
    init_logger();
    info!("比特币节点模拟程序启动");

    let nodes = Node::create_nodes(NODE_COUNT);
    info!("创建了 {} 个节点", nodes.len());

    let final_nodes = run_node_tasks(nodes)?;
    print_final_state(&final_nodes);

    Ok(())
}

/// 初始化日志系统
fn init_logger() {
    let env = env_logger::Env::default()
        .filter_or("LOG_LEVEL", "warn")
        .write_style_or("LOG_STYLE", "always");
    env_logger::init_from_env(env);
}

/// 运行所有节点的任务
fn run_node_tasks(nodes: Vec<Node>) -> Result<Vec<Node>> {
    let final_nodes = Arc::new(Mutex::new(Vec::new()));
    let tasks: Vec<_> = nodes
        .into_iter()
        .map(|node| {
            let final_nodes = Arc::clone(&final_nodes);
            std::thread::spawn(move || -> Result<()> {
                let final_node = run_single_node(node)?;
                final_nodes.lock().unwrap().push(final_node);
                Ok(())
            })
        })
        .collect();

    for task in tasks {
        task.join().unwrap()?;
    }

    let final_nodes = Arc::try_unwrap(final_nodes)
        .map_err(|_| anyhow::anyhow!("Failed to unwrap Arc"))?
        .into_inner()
        .map_err(|_| anyhow::anyhow!("Failed to unwrap Mutex"))?;

    Ok(final_nodes)
}

/// 打印最终的区块链状态
fn print_final_state(nodes: &[Node]) {
    warn!("=== 最终区块链状态 ===");
    for node in nodes {
        if let Some(utxo_set) = &node.utxo_set {
            let blockchain = &utxo_set.blockchain;
            warn!("节点 {}: ", node.id);
            warn!("  区块链长度: {}", blockchain.length);
            warn!("  最新区块哈希: {}", blockchain.tip);
            warn!("  区块链为 \n{}", blockchain.to_hashes_string());
            warn!("  账户地址: {}", node.wallets.get_addresses()[0]);
        }
    }
    warn!("所有节点任务完成");
}

/// 运行单个节点的任务
fn run_single_node(mut node: Node) -> Result<Node> {
    info!("节点 {} 开始运行", node.id);
    node.send_address_to_all();
    random_sleep(INITIAL_WAIT_TIME);

    for round in 0..MINING_ROUNDS {
        process_mining_round(&mut node, round)?;
    }

    FINISHED_COUNT.fetch_add(1, Ordering::SeqCst);
    info!("节点 {} 完成所有任务", node.id);
    wait_for_other_nodes(&mut node);

    Ok(node)
}

/// 处理单轮挖矿任务
fn process_mining_round(node: &mut Node, round: usize) -> Result<()> {
    debug!("节点 {} 开始第 {} 轮任务", node.id, round + 1);
    wait_for_message(node);

    initialize_blockchain_if_needed(node);
    wait_for_addresses(node);
    process_transactions(node, round);

    if try_collect_transactions(node) {
        mine_block(node);
    }

    Ok(())
}

/// 如果需要，初始化区块链
fn initialize_blockchain_if_needed(node: &mut Node) {
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
            node.send_message_to_all(Message::new(
                node.id,
                MessageData::BlockChain {
                    block_chain_length: 1,
                    block_chain: vec![genesis],
                },
            ));
        }
    }
}

/// 等待地址列表非空
fn wait_for_addresses(node: &mut Node) {
    while node.addresses.is_empty() {
        wait_for_message(node);
    }
}

/// 处理交易
fn process_transactions(node: &mut Node, round: usize) {
    for _ in 0..(MINING_ROUNDS - round) {
        if let Some(amount) = generate_random_transaction_amount() {
            if let Some(to_id) = select_random_recipient(node) {
                if check_balance(node, amount) {
                    debug!("节点 {} 向节点 {} 转账 {} 个币", node.id, to_id, amount);
                    node.send(to_id, amount);
                }
            }
        }
    }
}

/// 生成随机交易金额
fn generate_random_transaction_amount() -> Option<u32> {
    Some(rand::thread_rng().gen_range(MIN_TRANSACTION_AMOUNT..=MAX_TRANSACTION_AMOUNT))
}

/// 随机选择接收方
fn select_random_recipient(node: &Node) -> Option<usize> {
    let to_ids = node.addresses.keys().cloned().collect::<Vec<_>>();
    if to_ids.is_empty() {
        return None;
    }
    Some(to_ids[rand::thread_rng().gen_range(0..to_ids.len())])
}

/// 检查余额是否足够
fn check_balance(node: &Node, amount: u32) -> bool {
    if let Some(utxo_set) = &node.utxo_set {
        let balance =
            utxo_set.get_balance(node.wallets.get_addresses()[0], &Node::path_prefix(node.id));
        balance >= amount
    } else {
        false
    }
}

/// 尝试收集足够的交易
fn try_collect_transactions(node: &mut Node) -> bool {
    let start_time = Instant::now();
    let timeout = Duration::from_millis(TRANSACTION_WAIT_TIMEOUT);

    node.clean_invalid_transaction();
    while node.transaction_cache.len() < MIN_TRANSACTIONS_TO_MINE && start_time.elapsed() < timeout
    {
        debug!(
            "节点 {} 等待交易，当前缓存交易数: {}",
            node.id,
            node.transaction_cache.len()
        );
        wait_for_message(node);
        node.clean_invalid_transaction();
    }

    !node.transaction_cache.is_empty()
}

/// 挖掘新区块
fn mine_block(node: &mut Node) {
    info!("节点 {} 开始挖矿", node.id);
    node.mine();
    info!("节点 {} 完成挖矿", node.id);
}

/// 随机休眠一段时间
fn random_sleep(max_duration: u64) {
    let sleep_time = rand::thread_rng().gen_range(0..=max_duration);
    std::thread::sleep(Duration::from_millis(sleep_time));
}

/// 等待消息
fn wait_for_message(node: &mut Node) {
    let duration = Duration::from_millis(rand::thread_rng().gen_range(0..=MESSAGE_WAIT_TIME));
    let start_time = Instant::now();

    while start_time.elapsed() < duration {
        if let Ok(message) = node
            .recv_channel
            .recv_timeout(duration.saturating_sub(start_time.elapsed()))
        {
            let from_id = message.from_id;
            debug!("节点 {} 收到来自节点 {} 的消息", node.id, from_id);
            message.handle(node, from_id);
        }
    }
}

/// 等待其他节点完成
fn wait_for_other_nodes(node: &mut Node) {
    while FINISHED_COUNT.load(Ordering::SeqCst) < NODE_COUNT {
        if let Ok(message) = node
            .recv_channel
            .recv_timeout(Duration::from_millis(MESSAGE_HANDLE_TIMEOUT))
        {
            let from_id = message.from_id;
            warn!(
                "wait_for_handle: 节点 {} 收到来自节点 {} 的消息",
                node.id, from_id
            );
            message.handle(node, from_id);
        }
    }
}
