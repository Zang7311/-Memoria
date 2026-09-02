// 《铃·记忆体》默认配置（AI-7 任务 5）
// 首次启动时用默认值创建配置文件；缺失字段在迁移时也以默认值补齐。
use crate::types::AppConfig;
use crate::config;

/// 返回完整的默认配置
pub fn default_config() -> AppConfig {
    AppConfig {
        config_version: 1,
        theme: "dark".to_string(),
        context_length: 10,
        api_base_url: None,
        api_key_encrypted: None,
        api_key_plain: None,
        api_model: "gpt-3.5-turbo".to_string(),
        model_mode: "script".to_string(),
        depth: 2,
        language_mix_rate: 8,
        floating_ball_mode: "avatar".to_string(),
        floating_ball_enabled: true,
        floating_ball_size: 200,
        floating_ball_opacity: 1.0,
        floating_ball_breathing: true,
        floating_ball_flash: true,
        floating_ball_position: (0, 0),
        monitor_enabled: true,
        monitor_frequency: 3,
        // ⚠️ 架构债务：monitor_rules / toolbox_items 归 AI-6 管理，AI-7 默认空
        monitor_rules: Vec::new(),
        toolbox_items: Vec::new(),
        hotkey: "Ctrl+Alt+L".to_string(),
        autostart: false,
        data_path: config::default_data_path(),
        first_launch: true,
        plugin_enabled: true,
        enabled_plugins: Vec::new(),
        self_name: Some("铃".to_string()),
        user_name: Some("主人".to_string()),
        persona: "daily".to_string(),
        master_password_salt: None,
        has_master_password: false,
        master_password_check: None,
        accent_color: None,
        danger_color: None,
        bg_color: None,
        bg_image: None,
        avatar_suzu: None,
        avatar_user: None,
        ui_radius: None,
        bubble_user_color: None,
        bubble_suzu_color: None,
        ui_themes: None,
        run_as_admin: false,
        emoji_mode: None,
        ai_toolbox: false,
        // —— AI-9 快捷指令（默认空，用户自行创建）——
        quick_commands: Vec::new(),
        search_mode: "bigram".to_string(),
    }
}
