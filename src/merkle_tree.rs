use crate::{block::Hash, transaction::Transaction};
use sha2::{Digest, Sha256};

/// 默克尔树的实现
///
/// 使用数组存储树节点，每个节点可以是 None（空节点）或 Some(Hash)（有效节点）
/// 对于 n 个叶子节点，数组大小为 2n-1，其中：
/// - 前 n 个元素为叶子节点（交易哈希）
/// - 后 n-1 个元素为内部节点（两个子节点的哈希组合）
pub struct MerkleTree(Vec<Option<Hash>>);

impl MerkleTree {
    /// 从交易列表创建默克尔树
    ///
    /// # 算法步骤
    /// 1. 计算需要的数组大小（2 * next_power_of_two - 1）
    /// 2. 将交易哈希填充到数组前面
    /// 3. 自底向上构建树，每个父节点是其两个子节点的组合哈希
    pub fn new(transactions: Vec<Transaction>) -> Self {
        let next_pow_of_two = transactions.len().next_power_of_two();
        let array_size = next_pow_of_two * 2 - 1;
        let mut array = vec![None; array_size];

        // 填充叶子节点（交易哈希）
        for (index, item) in transactions.iter().enumerate() {
            array[index] = Some(item.id);
        }

        // 从第一个非叶子节点开始构建树
        let mut offset = next_pow_of_two;

        // 每次处理两个子节点，计算它们的父节点
        for i in (0..array_size - 1).step_by(2) {
            match (&array[i], &array[i + 1]) {
                // 两个子节点都为空，父节点也为空
                (None, None) => {
                    array[offset] = None;
                }
                // 只有左子节点，父节点是左子节点的自我哈希
                (Some(a), None) => {
                    array[offset] = Some(Hash::from(&Sha256::digest(
                        [a.as_bytes(), a.as_bytes()].concat(),
                    )));
                }
                // 两个子节点都存在，父节点是它们的组合哈希
                (Some(a), Some(b)) => {
                    array[offset] = Some(Hash::from(&Sha256::digest(
                        [a.as_bytes(), b.as_bytes()].concat(),
                    )));
                }
                // 左空右非空的情况在正确构建的树中不应该出现
                (None, Some(_)) => {
                    panic!("right child is not None but left child is None");
                }
            }
            offset += 1;
        }

        Self(array)
    }

    /// 获取默克尔树的根哈希
    ///
    /// 返回数组最后一个元素（根节点）的哈希值
    /// 如果树为空，返回 None
    pub fn root(&self) -> Option<Hash> {
        self.0.last().cloned().flatten()
    }
}
