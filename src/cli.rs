#![allow(unused)]

use crate::network::Node;
use crate::transaction::Transaction;
use crate::utxo_set::UtxoSet;
use crate::{wallet, CliMessage, NODE_COUNT};
use anyhow::{Context, Result};
use clap::{self, Subcommand};
use clap::{command, Parser};
use log::info;
use std::io::{self, Write};
use std::sync::mpsc::Sender;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// 初始化新的区块链
    Init,

    /// 自动运行测试
    AutoRun {
        #[clap(short, long)]
        is_interactive: bool,
    },

    /// 清理数据
    Clean,
}

// 定义交互模式专用的命令
#[derive(Parser)]
pub enum InteractiveCommands {
    /// 发送交易
    Send {
        #[clap(short, long)]
        from_node_id: usize,
        #[clap(short, long)]
        to_node_id: usize,
        #[clap(short, long)]
        amount: u32,
    },

    /// 获取余额
    Balance {
        #[clap(short, long)]
        node_id: usize,
    },

    /// 列出所有钱包地址
    Wallets {
        #[clap(short, long)]
        node_id: usize,
    },

    /// 打印区块链
    Chain {
        #[clap(short, long)]
        node_id: usize,
    },

    /// 验证区块链完整性
    Verify {
        #[clap(short, long)]
        node_id: usize,
    },

    /// 退出程序
    Exit,

    /// 继续
    Continue,
}

pub fn run_cli(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init => {
            Node::clean_data(NODE_COUNT);
            let nodes = Node::create_nodes(NODE_COUNT);
            info!(target: "chain", "创建了 {} 个节点", nodes.0.len());
            println!("区块链初始化成功");
        }
        Commands::AutoRun { is_interactive } => {
            crate::auto_run(is_interactive)?;
            println!("自动运行测试完成");
        }
        Commands::Clean => {
            Node::clean_data(NODE_COUNT);
            println!("数据清理完成");
        }
    }
    Ok(())
}

pub fn run_interactive_cli(cli_send_channels: Vec<Sender<CliMessage>>) -> Result<()> {
    println!("输入 help 查看可用命令");

    loop {
        print!("> ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        // 特殊处理 help 命令
        if input == "help" {
            print_help();
            continue;
        }

        // 将输入转换为参数数组，添加程序名作为第一个参数
        let args = std::iter::once("cli")
            .chain(input.split_whitespace())
            .collect::<Vec<_>>();

        match InteractiveCommands::try_parse_from(args) {
            Ok(command) => match command {
                InteractiveCommands::Send {
                    from_node_id,
                    to_node_id,
                    amount,
                } => {
                    if let Some(channel) = cli_send_channels.get(from_node_id) {
                        channel.send(CliMessage::Send { to_node_id, amount })?;
                        for channel in cli_send_channels.iter() {
                            channel.send(CliMessage::Continue)?;
                        }
                    }
                }
                InteractiveCommands::Balance { node_id } => {
                    if let Some(channel) = cli_send_channels.get(node_id) {
                        channel.send(CliMessage::PrintBalance)?;
                    }
                }
                InteractiveCommands::Chain { node_id } => {
                    if let Some(channel) = cli_send_channels.get(node_id) {
                        channel.send(CliMessage::PrintChain)?;
                    }
                }
                InteractiveCommands::Verify { node_id } => {
                    if let Some(channel) = cli_send_channels.get(node_id) {
                        channel.send(CliMessage::VerifyChain)?;
                    }
                }
                InteractiveCommands::Wallets { node_id } => {
                    if let Some(channel) = cli_send_channels.get(node_id) {
                        channel.send(CliMessage::ListWallets)?;
                    }
                }
                InteractiveCommands::Continue => {
                    for channel in cli_send_channels.iter() {
                        channel.send(CliMessage::Continue)?;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }
                InteractiveCommands::Exit => {
                    std::process::exit(0);
                }
            },
            Err(e) => {
                println!("命令解析错误: {}", e);
            }
        }
    }
    Ok(())
}

fn print_help() {
    println!("可用命令：");
    println!("  help                      显示此帮助信息");
    println!("  send -f <from> -t <to> -a <amount>  发送交易");
    println!("  balance -n <node_id>      查看节点余额");
    println!("  chain -n <node_id>        打印区块链");
    println!("  verify -n <node_id>       验证区块链");
    println!("  wallets -n <node_id>      列出钱包地址");
    println!("  exit                      退出程序");
}

pub fn parse_cli() -> Cli {
    Cli::parse()
}
