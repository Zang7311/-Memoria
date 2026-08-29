// 《铃·记忆体》Tauri 入口：注册所有命令与模块
mod commands;
mod config;
mod deps;
mod context;
mod desktop;
mod diagnostic;
mod engine;
mod error;
mod logs;
mod memory;
mod network;
mod sessions;
// AI-5：插件模块与类型公开（供集成测试/下游引用）
pub mod plugin;
mod stream;
mod sync;
pub mod types;
mod update;
mod utils;

use tauri::Manager;

// —— 测试通道（保留，供前端 greet 使用）——
#[tauri::command]
fn greet(name: &str) -> String {
    format!("你好，{}！铃已经准备好了。", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // —— AI-6：全局快捷键插件 ——
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // —— AI-7 配置中心 + 日志系统（最先初始化，供其他模块使用）——
            logs::init();
            config::store::init(app.handle().clone());
            log::info!("[setup] 铃·记忆体 启动，配置与日志系统已就绪");

            // 用户开启「始终以管理员运行」且当前非管理员 → 自动提权重启（弹一次 UAC）
            if config::store::get_config().run_as_admin && !commands::admin::is_admin() {
                log::info!("[setup] 「始终以管理员运行」已开启且当前为普通权限，自动提权重启…");
                if commands::admin::restart_as_admin().is_ok() {
                    std::process::exit(0);
                }
            }

            // —— AI-8 同步模块初始化 + TCP 监听 + 网络监测 ——
            sync::init();
            sync::spawn_listener(app.handle().clone());
            network::spawn_monitor(app.handle().clone());

            // —— AI-5 插件系统初始化 ——
            plugin::init(&app.handle());

            // ==================== AI-6 桌面交互初始化 ====================

            // 1. 系统托盘（常驻右下角，右键菜单）
            desktop::tray::init_tray(app.handle());

            // 2. 悬浮球窗口（独立无边框透明小窗，常驻置顶；默认隐藏，托盘/设置可开启）
            tauri::WebviewWindowBuilder::new(
                app,
                "floating-ball",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("铃·记忆体 悬浮球")
            .inner_size(80.0, 80.0)
            .min_inner_size(80.0, 80.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .visible(false)
            .build()?;

            // 3. 气泡弹窗窗口（右下角提示；默认隐藏，监测触发时显示）
            tauri::WebviewWindowBuilder::new(
                app,
                "bubble",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("铃 气泡")
            .inner_size(380.0, 120.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .visible(false)
            .build()?;

            // 4. 屏幕监测后台任务（轮询前台窗口 + 规则匹配 + 三层兜底）
            desktop::monitor::start_monitor(app.handle().clone());

            // 5. 全局快捷键：默认 Ctrl+Alt+L 呼出/隐藏主窗口（失败不阻塞启动）
            if let Err(e) = desktop::hotkey::register(
                app.handle(),
                desktop::hotkey::DEFAULT_ACCELERATOR,
            ) {
                log::warn!("[setup] 默认快捷键注册失败：{e}");
            }

            // 6. 开机自启动场景（--minimized）：隐藏主窗口，静默驻留托盘
            let args: Vec<String> = std::env::args().collect();
            if args.iter().any(|a| a == "--minimized") {
                if let Some(win) = app.get_webview_window("main") {
                    let _ = win.hide();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::send_message::send_message,
            commands::test_connection::test_api_connection,
            // —— AI-4 记忆命令 ——
            commands::memory_get::get_memories,
            commands::memory_delete::delete_memory,
            commands::memory_set_switch::switch_memory_set,
            commands::memory_set_create::create_memory_set,
            commands::memory_set_list::list_memory_sets,
            commands::memory_write::write_memory,
            commands::memory_mark::mark_memory_important,
            // —— AI-5 插件命令 ——
            commands::plugin_list::list_plugins,
            commands::plugin_install::install_plugin,
            commands::plugin_uninstall::uninstall_plugin,
            commands::plugin_enable::enable_plugin,
            commands::plugin_enable::disable_plugin,
            commands::plugin_execute::execute_skill,
            commands::plugin_permission::set_plugin_permission,
            commands::plugin_terminal::add_terminal_command,
            // —— AI-6 桌面交互命令 ——
            commands::window_info::get_window_info,
            commands::monitor_rules::get_monitor_rules,
            commands::monitor_rules::update_monitor_rule,
            commands::monitor_rules::delete_monitor_rule,
            commands::monitor_rules::toggle_monitoring,
            commands::toolbox_execute::list_toolbox_items,
            commands::toolbox_execute::execute_toolbox,
            commands::toolbox_execute::save_toolbox_item,
            commands::toolbox_execute::delete_toolbox_item,
            commands::pixel_art::save_pixel_art,
            commands::ui_image::save_ui_image,
            deps::check_dependency,
            commands::tray::set_floating_ball_visibility,
            commands::hotkey::register_hotkey,
            commands::hotkey::unregister_hotkey,
            commands::autostart::set_autostart,
            commands::autostart::get_autostart,
            // —— AI-7 配置与诊断命令 ——
            commands::config_get::get_config,
            commands::config_update::update_config,
            commands::config_export::export_config,
            commands::config_import::import_config,
            commands::config_reset::reset_config,
            commands::logs_get::get_logs,
            commands::logs_clear::clear_logs,
            commands::diagnostic_export::export_diagnostic,
            commands::system_info::get_system_info,
            commands::master::set_master_password,
            commands::master::unlock,
            commands::master::master_password_status,
            // —— AI-8 网络与同步命令 ——
            commands::sync_discover::discover_devices,
            commands::sync_discover::add_manual_device,
            commands::sync_start::start_sync,
            commands::sync_status::get_sync_status,
            commands::sync_status::set_sync_password,
            commands::sync_status::set_conflict_policy,
            commands::sync_status::get_sync_devices,
            commands::update_check::check_update,
            commands::network_status::get_network_status,
            commands::open_url::open_url,
            // —— 收尾工程师批次3：多会话 ——
            commands::session::session_list,
            commands::session::session_create,
            commands::session::session_load,
            commands::session::session_save,
            commands::session::session_rename,
            commands::session::session_delete,
            // —— 收尾工程师：管理员权限 ——
            commands::admin::is_admin,
            commands::admin::restart_as_admin,
            // —— 收尾工程师：一键本地部署 AI ——
            commands::local_ai::detect_ollama,
            commands::local_ai::pull_model,
            commands::local_ai::detect_gpu_vram,
            commands::local_ai::set_ollama_models_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
