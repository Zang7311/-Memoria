// 《铃·记忆体》VBIL 模块 —— 客户端管理（ClientManager）
//
// 维护在线客户端列表（HashMap：id → ClientInfo），供 TCP 服务端、心跳任务并发访问。
// 并发安全：内部用 tokio::sync::Mutex 包裹。

use crate::vbil::types::MISSED_PONG_LIMIT;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use tokio::sync::Mutex;

/// 在线客户端信息
#[derive(Debug, Clone)]
pub struct ClientInfo {
    /// 实例唯一 ID
    pub id: String,
    /// 显示名称
    pub name: Option<String>,
    /// 能力清单
    pub capabilities: Vec<String>,
    /// 最近一次心跳时间
    pub last_heartbeat: DateTime<Utc>,
    /// 建立连接时间
    pub connected_at: DateTime<Utc>,
    /// 连续未响应 ping 的次数（用于超时判定）
    pub missed_pongs: u32,
}

/// 客户端管理器
pub struct ClientManager {
    clients: Mutex<HashMap<String, ClientInfo>>,
}

impl ClientManager {
    /// 创建空的管理器
    pub fn new() -> Self {
        Self {
            clients: Mutex::new(HashMap::new()),
        }
    }

    /// 注册客户端；若同 id 已存在则覆盖并返回 false，否则返回 true
    pub async fn add_client(
        &self,
        id: String,
        name: Option<String>,
        capabilities: Vec<String>,
    ) -> bool {
        let mut map = self.clients.lock().await;
        let now = Utc::now();
        let info = ClientInfo {
            id: id.clone(),
            name,
            capabilities,
            last_heartbeat: now,
            connected_at: now,
            missed_pongs: 0,
        };
        map.insert(id, info).is_none()
    }

    /// 移除客户端；返回被移除的信息（若存在）
    pub async fn remove_client(&self, id: &str) -> Option<ClientInfo> {
        self.clients.lock().await.remove(id)
    }

    /// 查询客户端
    pub async fn get_client(&self, id: &str) -> Option<ClientInfo> {
        self.clients.lock().await.get(id).cloned()
    }

    /// 列出全部在线客户端
    pub async fn list_clients(&self) -> Vec<ClientInfo> {
        self.clients.lock().await.values().cloned().collect()
    }

    /// 更新心跳（收到 pong 时调用）；返回该客户端是否存在
    pub async fn update_heartbeat(&self, id: &str) -> bool {
        let mut map = self.clients.lock().await;
        if let Some(c) = map.get_mut(id) {
            c.last_heartbeat = Utc::now();
            c.missed_pongs = 0;
            true
        } else {
            false
        }
    }

    /// 标记一次 ping 未响应（心跳超时轮询时调用）。
    /// 返回 Some(true) 表示已达超时阈值，应由调用方断开并移除；None 表示客户端不存在。
    pub async fn mark_missed_pong(&self, id: &str) -> Option<bool> {
        let mut map = self.clients.lock().await;
        if let Some(c) = map.get_mut(id) {
            c.missed_pongs += 1;
            Some(c.missed_pongs >= MISSED_PONG_LIMIT)
        } else {
            None
        }
    }

    /// 移除所有连续未响应达到阈值的客户端，返回被移除的 id 列表
    pub async fn check_stale_clients(&self) -> Vec<String> {
        let mut map = self.clients.lock().await;
        let stale: Vec<String> = map
            .values()
            .filter(|c| c.missed_pongs >= MISSED_PONG_LIMIT)
            .map(|c| c.id.clone())
            .collect();
        for id in &stale {
            map.remove(id);
        }
        stale
    }
}

impl Default for ClientManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> ClientManager {
        ClientManager::new()
    }

    #[tokio::test]
    async fn add_and_get_client() {
        let m = manager();
        let added = m
            .add_client(
                "id-1".into(),
                Some("SSP桌宠".into()),
                vec!["event.send".into()],
            )
            .await;
        assert!(added);
        let c = m.get_client("id-1").await.unwrap();
        assert_eq!(c.name.as_deref(), Some("SSP桌宠"));
        assert_eq!(c.capabilities, vec!["event.send".to_string()]);
    }

    #[tokio::test]
    async fn remove_client() {
        let m = manager();
        m.add_client("id-1".into(), None, vec![]).await;
        assert!(m.remove_client("id-1").await.is_some());
        assert!(m.get_client("id-1").await.is_none());
    }

    #[tokio::test]
    async fn heartbeat_resets_missed_pongs() {
        let m = manager();
        m.add_client("id-1".into(), None, vec![]).await;
        // 模拟一次未响应
        assert_eq!(m.mark_missed_pong("id-1").await, Some(false));
        // 收到 pong 后清零
        assert!(m.update_heartbeat("id-1").await);
        let c = m.get_client("id-1").await.unwrap();
        assert_eq!(c.missed_pongs, 0);
    }

    #[tokio::test]
    async fn stale_client_removed_after_limit() {
        let m = manager();
        m.add_client("id-1".into(), None, vec![]).await;
        m.add_client("id-2".into(), None, vec![]).await;

        // id-1 连续 MISSED_PONG_LIMIT 次未响应
        for i in 1..=MISSED_PONG_LIMIT {
            let res = m.mark_missed_pong("id-1").await;
            if i < MISSED_PONG_LIMIT {
                assert_eq!(res, Some(false));
            } else {
                assert_eq!(res, Some(true));
            }
        }

        // id-2 只未响应一次，不应被移除
        m.mark_missed_pong("id-2").await;

        let removed = m.check_stale_clients().await;
        assert_eq!(removed, vec!["id-1".to_string()]);
        assert!(m.get_client("id-1").await.is_none());
        assert!(m.get_client("id-2").await.is_some());
    }

    #[tokio::test]
    async fn list_clients_returns_all() {
        let m = manager();
        m.add_client("id-1".into(), None, vec![]).await;
        m.add_client("id-2".into(), None, vec![]).await;
        assert_eq!(m.list_clients().await.len(), 2);
    }
}
