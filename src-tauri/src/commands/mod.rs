// 《铃·记忆体》commands 模块
pub mod send_message;
pub mod test_connection;
// —— AI-4 记忆命令 ——
pub mod memory_delete;
pub mod memory_get;
pub mod memory_mark;
pub mod memory_set_create;
pub mod memory_set_list;
pub mod memory_set_switch;
pub mod memory_write;
// —— AI-5 插件命令 ——
pub mod plugin_enable;
pub mod plugin_execute;
pub mod plugin_install;
pub mod plugin_list;
pub mod plugin_permission;
pub mod plugin_terminal;
pub mod plugin_uninstall;
// —— AI-6 桌面交互命令 ——
pub mod autostart;
pub mod hotkey;
pub mod monitor_rules;
pub mod toolbox_execute;
pub mod tray;
pub mod window_info;
pub mod pixel_art;
pub mod ui_image;
// —— AI-7 配置与诊断命令 ——
pub mod config_export;
pub mod config_get;
pub mod config_import;
pub mod config_reset;
pub mod config_update;
pub mod diagnostic_export;
pub mod logs_clear;
pub mod logs_get;
pub mod master;
pub mod system_info;
// —— AI-8 网络与同步命令 ——
pub mod network_status;
pub mod sync_discover;
pub mod sync_start;
pub mod sync_status;
pub mod update_check;
// —— 收尾工程师 ——
pub mod open_url;
pub mod session;
pub mod admin;
pub mod local_ai;
// —— AI-9 快捷指令系统 ——
pub mod quick_command;
// —— moon10 二维码生成与识别 ——
pub mod qrcode;
// —— moon11 OCR 文字识别 ——
pub mod ocr;
// —— 离线检索增强：搜索引擎模式配置 ——
pub mod search_mode;
