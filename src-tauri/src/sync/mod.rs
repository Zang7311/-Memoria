// 《铃·记忆体》同步模块（AI-8）—— 导出同步子模块
//
// 功能：局域网设备发现（UDP）、手动连接备选、加密数据传输（TCP）、
//       增量/全量同步、冲突检测与解决、同步状态与历史。
pub mod conflict;
pub mod discovery;
pub use discovery::spawn_responder;
pub mod encryption;
pub mod payload;
pub mod transfer;

pub use transfer::{spawn_listener, DATA_TRANSFER_PORT};

/// 设备发现端口（任务书固定 54545，可配置）
pub const DEVICE_DISCOVERY_PORT: u16 = 54545;

/// 初始化同步模块（lib.rs setup 调用）
pub fn init() {
    conflict::init();
    discovery::load_devices();
}
