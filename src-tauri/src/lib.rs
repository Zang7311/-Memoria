// 《铃·记忆体》Tauri 入口：注册所有命令与模块
mod commands;
mod audit;
mod attachments;
#[cfg(test)]
mod attachment_tests;
#[cfg(test)]
mod model_slot_tests;
mod config;
mod deps;
mod context;
mod desktop;
mod diagnostic;
mod engine;
mod error;
mod events;
mod logs;
mod memory;
mod milestones;
mod network;
mod sessions;
// AI-5：插件模块与类型公开（供集成测试/下游引用）
pub mod plugin;
mod stream;
mod sync;
pub mod types;
mod update;
mod utils;
mod vbil;
// AI-10：Agent 能力增强（工具描述生成器 + Agent 循环）
pub mod agent;

use tauri::Manager;
use tauri::Emitter;
use tauri::Listener;

// —— 测试通道（debug 构建返回实际内容；release 构建为空操作，命令仍注册但无实质功能）——
#[tauri::command]
fn greet(name: &str) -> String {
    #[cfg(debug_assertions)]
    return format!("你好，{}！铃已经准备好了。", name);
    #[cfg(not(debug_assertions))]
    { let _ = name; String::new() }
}

/// 桌宠窗口的启动位置（逻辑坐标）。
///
/// 配置里存过位置就用配置；全 0（从未移动过）→ 屏幕右下角，别一上来就压住主窗口。
/// 目前仅作为参考实现保留：窗口位置统一由前端在「窗口稳定后」恢复，
/// 因为透明窗口在创建阶段被移动会导致画面与外壳错位（见 setup 里的注释）。
#[allow(dead_code)]
fn floating_ball_start_position(app: &tauri::AppHandle) -> (f64, f64) {
    const WIN_W: f64 = 300.0;
    const WIN_H: f64 = 464.0;
    let (x, y) = config::store::get_config().floating_ball_position;
    if x != 0 || y != 0 {
        return (x as f64, y as f64);
    }
    let monitor = app.primary_monitor().ok().flatten();
    let scale = monitor.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let (sw, sh) = monitor
        .as_ref()
        .map(|m| (m.size().width as f64 / scale, m.size().height as f64 / scale))
        .unwrap_or((1280.0, 720.0));
    ((sw - WIN_W - 24.0).max(0.0), (sh - WIN_H - 48.0).max(0.0))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // —— 主窗口/悬浮球卡顿修复：WebView2 强制软件渲染（老显卡 R7 350 GPU 合成不稳定）
    // 2026-10-17 试过启用它排查桌宠窗口错位：无效（软件渲染下依然错位），已恢复注释。
    // 注意：必须在任何 webview 创建前设置；若报 0x8007139F 通常是残留实例占用
    // WebView2 user data 锁（先杀干净 mem 进程再启动），非本参数问题。
    // std::env::set_var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--disable-gpu");
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // —— AI-6：全局快捷键插件 ——
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // —— AI-7 配置中心 + 日志系统（最先初始化，供其他模块使用）——
            logs::init();

            // ==================== P3 救援模式 ====================
            // 触发条件：启动参数 --recovery（配置损坏/启动失败时用户手动进入）
            config::store::init(app.handle().clone());
            agent::forged_tools::init();
            let recovery_mode = std::env::args().any(|a| a == "--recovery");
            if recovery_mode {
                // 恢复模式：只建极简窗口，跳过全部正常初始化
                log::warn!("[setup] 进入恢复模式（--recovery 或配置初始化失败）");
                tauri::WebviewWindowBuilder::new(
                    app,
                    "recovery",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("铃·恢复模式")
                .inner_size(720.0, 520.0)
                .resizable(false)
                .build()?;
                return Ok(());
            }

            log::info!("[setup] 铃·记忆体 启动，配置与日志系统已就绪");

            // 诊断用：主窗口被销毁/请求关闭时留一条日志。
            // 2026-10-31 测试中主窗口曾两次“无声消失”（进程还在、窗口没了、无报错），
            // 有了这条日志下次能立刻分清是业务代码关的、还是 WebView 崩了之后被收掉的。
            if let Some(win) = app.get_webview_window("main") {
                win.on_window_event(|event| match event {
                    tauri::WindowEvent::Destroyed => log::warn!("[window] 主窗口被销毁（Destroyed）"),
                    tauri::WindowEvent::CloseRequested { .. } => log::info!("[window] 主窗口收到关闭请求"),
                    _ => {}
                });
            }

            // 主窗口尺寸按屏幕自适应：高缩放的笔记本上不会顶出屏幕、底部图标被任务栏挡住
            desktop::window_fit::fit_main_window(app.handle());

            // 用户开启「始终以管理员运行」且当前非管理员 → 自动提权重启（弹一次 UAC）
            if config::store::get_config().run_as_admin && !commands::admin::is_admin() {
                log::info!("[setup] 「始终以管理员运行」已开启且当前为普通权限，自动提权重启…");
                if commands::admin::restart_as_admin().is_ok() {
                    std::process::exit(0);
                }
            }

            // WAL 回放：将上次崩溃前未 checkpoint 的追加记录合并回 index.json
            {
                let default_path = memory::storage::default_index_path();
                if let Err(e) = memory::wal::replay_wal(&default_path) {
                    log::warn!("[wal] default 集回放失败（不影响启动）：{e}");
                }
            }

            // —— AI-8 同步模块初始化 + TCP 监听 + 网络监测 ——
            sync::init();
            sync::spawn_listener(app.handle().clone());
            sync::spawn_responder();
            network::spawn_monitor(app.handle().clone());

            // —— VBIL 虚拟形象互联层初始化（本地 TCP 服务 + 心跳 + 规则引擎 + 响应桥接）——
            vbil::init();
            vbil::spawn_listener(app.handle().clone());
            vbil::rules::init();
            vbil::spawn_responder(app.handle().clone());

            // —— AI-5 插件系统初始化 ——
            plugin::init(&app.handle());

            // ==================== AI-6 桌面交互初始化 ====================

            // 1. 系统托盘（常驻右下角，右键菜单）
            desktop::tray::init_tray(app.handle());

            // 2. 悬浮球窗口（独立无边框透明小窗，常驻置顶；默认隐藏，托盘/设置可开启）
            //    默认 200x200（支持 Live2D），最小 80x80（纯头像/文字模式）
            //
            // ⚠️ 这里刻意不给 .position()：透明窗口在 WebView2 内容初始化前后被移动，
            //    会出现「外壳移走了、画面留在原地（甚至跑到屏幕外）」的错位
            //    （2026-10-17 实机踩到）。位置由前端在窗口稳定后恢复，见 FloatingBall.vue。
            tauri::WebviewWindowBuilder::new(
                app,
                "floating-ball",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("铃·记忆体 悬浮球")
            // 先按小尺寸创建、由前端在挂载后 setSize 到目标尺寸：
            // 高 DPI 下「创建即最终尺寸」会让 oml2d 的 canvas 按物理像素撑开、溢出窗口；
            // 走一次 setSize 能让 WebView 重新布局，canvas 才会贴合窗口。
            .inner_size(200.0, 200.0)
            .min_inner_size(80.0, 80.0)
            .resizable(false)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .visible(false)
            .build()?;

            // 桌宠是常驻的：配置里开着「显示桌宠」就随应用启动出现，
            // 不该每次手动去点托盘。首次安装默认开启。
            if config::store::get_config().floating_ball_enabled {
                if let Some(ball) = app.get_webview_window("floating-ball") {
                    let _ = ball.show();
                }
            }

            // 原生桌宠（独立进程 + Live2D）：配置里开着就随之起来。
            // 找不到程序（开发态没构建）只是不启动，不影响应用本身。
            match desktop::native_pet::sync_with_config(app.handle()) {
                Ok(st) => {
                    if st.running {
                        log::info!("[native-pet] {}", st.message);
                    } else {
                        log::info!("[native-pet] 未启动：{}", st.message);
                    }
                }
                Err(e) => log::warn!("[native-pet] 启动失败：{e}"),
            }

            // 临时调试通道（v0.6 悬浮球诊断，发布前移除）：前端 emit ball-diag → 打印到日志
            {
                let handle = app.handle().clone();
                handle.listen("ball-diag", move |ev| {
                    log::info!("[ball-diag] {}", ev.payload());
                });
            }

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

            if agent::goals::start(app.handle().clone()).is_err() {
                log::warn!("目标存储初始化失败，自动推进未启动，请检查 goals.json");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::audit::list_audit,
            commands::audit::export_audit,
            commands::audit::clear_audit,
            greet,
            // —— AI-10 Agent 命令 ——
            commands::agent_run::agent_run,
            commands::agent_run::agent_plan_respond,
            commands::agent_cancel::agent_cancel,
            commands::goals::list_goals,
            commands::goals::create_goal,
            commands::goals::goal_advance,
            commands::goals::set_goal_status,
            commands::goals::update_goal_settings,
            commands::goals::delete_goal,
            commands::send_message::send_message,
            commands::forged_tools::list_forged_tools,
            commands::forged_tools::delete_forged_tool,
            commands::forged_tools::set_forged_tool_enabled,
            commands::test_connection::test_api_connection,
            commands::network_diagnostic::network_diagnostic,
            // —— AI-4 记忆命令 ——
            commands::memory_get::get_memories,
            commands::memory_delete::delete_memory,
            commands::memory_set_switch::switch_memory_set,
            commands::memory_set_create::create_memory_set,
            commands::memory_set_list::list_memory_sets,
            commands::memory_write::write_memory,
            commands::memory_mark::mark_memory_important,
            // —— 记忆中心命令 ——
            memory::center::memory_stats,
            memory::center::delete_memories_batch,
            memory::center::mark_important_batch,
            memory::center::edit_memory_content,
            // —— P3 陪伴记录命令 ——
            milestones::record_milestone,
            milestones::get_milestones,
            milestones::record_daily_chat,
            milestones::record_daily_tool,
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
            commands::main_window::ensure_main_window,
            commands::ball::set_floating_ball_click_through,
            commands::ball::get_floating_ball_click_through,
            commands::hotkey::register_hotkey,
            commands::hotkey::unregister_hotkey,
            commands::autostart::set_autostart,
            commands::autostart::get_autostart,
            // —— AI-7 配置与诊断命令 ——
            commands::config_get::get_config,
            commands::config_update::update_config,
            commands::model_slot::save_model_slot_key,
            commands::config_export::export_config,
            commands::config_import::import_config,
            commands::config_reset::reset_config,
            commands::logs_get::get_logs,
            commands::logs_clear::clear_logs,
            commands::diagnostic_export::export_diagnostic,
            // —— P3 救援模式命令 ——
            diagnostic::recovery_check,
            diagnostic::recovery_reset_config,
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
            // —— AI-9 快捷指令系统 ——
            commands::quick_command::list_quick_commands,
            commands::quick_command::save_quick_command,
            commands::quick_command::delete_quick_command,
            commands::quick_command::execute_quick_command,
            commands::quick_command::set_power_mode,
            commands::quick_command::set_volume,
            commands::quick_command::play_music,
            // —— moon10 二维码生成与识别 ——
            commands::qrcode::generate_qrcode,
            commands::qrcode::decode_qrcode,
            // —— moon11 OCR 文字识别 ——
            commands::ocr::ocr_image,
            // —— 离线检索增强：搜索引擎模式配置 ——
            commands::search_mode::get_search_mode,
            commands::search_mode::set_search_mode,
            commands::search_mode::check_vector_model_status,
            commands::search_mode::scan_model_files,
            commands::search_mode::install_model,
            // —— 应用生命周期 ——
            commands::app_lifecycle::quit_app,
            // —— 原生桌宠托管 ——
            desktop::native_pet::native_pet_status,
            desktop::native_pet::native_pet_sync,
            desktop::native_pet::native_pet_restart,
            desktop::pet_talk::native_pet_say,
            // —— 拖放附件 ——
            commands::read_dropped_file::read_dropped_file,
            // —— VBIL 虚拟形象互联层命令 ——
            vbil::commands::scan_windows,
            vbil::commands::get_vbil_status,
            vbil::commands::set_vbil_enabled,
            vbil::commands::set_vbil_mode,
            vbil::commands::set_whitelist,
            vbil::commands::get_online_clients,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            // 应用退了就把托管的桌宠一起收掉（桌宠自己也盯着父进程，双保险）
            if let tauri::RunEvent::Exit = event {
                desktop::native_pet::shutdown();
            }
        });
}
