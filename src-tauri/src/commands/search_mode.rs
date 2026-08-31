// 《铃·记忆体》搜索引擎模式配置命令
use crate::config::store;
use crate::memory::search::check_vector_model;
use serde::Serialize;
use std::path::PathBuf;

/// 获取当前搜索模式（"bigram" / "bm25" / "vector"）
#[tauri::command]
pub fn get_search_mode() -> String {
    store::get_config().search_mode
}

/// 设置搜索模式；非法值返回 Err
#[tauri::command]
pub fn set_search_mode(mode: String) -> Result<String, String> {
    match mode.as_str() {
        "bigram" | "bm25" | "vector" => {}
        _ => return Err(format!("不支持的搜索模式：{mode}，有效值：bigram / bm25 / vector")),
    }

    let mut updates = std::collections::HashMap::new();
    updates.insert(
        "search_mode".to_string(),
        serde_json::Value::String(mode.clone()),
    );
    store::update(&updates).map_err(|e| e.to_string())?;
    Ok(mode)
}

/// 检测向量模型安装状态（不加载模型）
#[tauri::command]
pub fn check_vector_model_status() -> crate::memory::search::VectorModelStatus {
    check_vector_model()
}

// ===== 模型扫描与一键安装 =====

#[derive(Serialize)]
pub struct ModelCandidate {
    pub path: String,
    pub filename: String,
    pub size_mb: f64,
    /// 文件已在目标库（~/.铃记忆体/models/）中
    pub exists_in_target: bool,
}

#[derive(Serialize)]
pub struct InstallModelResult {
    pub success: bool,
    pub message: String,
    pub missing_files: Vec<String>,
}

fn home_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    PathBuf::from(home)
}

fn target_models_dir() -> PathBuf {
    home_dir().join(".铃记忆体").join("models")
}

fn is_model_file(name: &str) -> bool {
    name.to_lowercase() == "model.safetensors"
}

/// 扫描常见位置的向量模型文件（model.safetensors / bge*.safetensors/onnx/gguf 等）
#[tauri::command]
pub fn scan_model_files() -> Vec<ModelCandidate> {
    let home = home_dir();
    let target = target_models_dir();

    let mut search_dirs: Vec<PathBuf> = vec![
        target.clone(),
        home.join(".ling-memoria").join("models"),
        home.join("Downloads"),
        home.join("Desktop"),
        home.join("Documents"),
    ];
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            search_dirs.push(exe_dir.join("models"));
        }
    }

    let mut candidates = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for dir in &search_dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let filename = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if !is_model_file(&filename) {
                continue;
            }
            let path_str = path.to_string_lossy().to_string();
            if !seen.insert(path_str.clone()) {
                continue;
            }
            let size_mb = entry
                .metadata()
                .map(|m| m.len() as f64 / 1_048_576.0)
                .unwrap_or(0.0);
            // 文件的父目录就是目标库则视为已安装
            let exists_in_target = path
                .parent()
                .map(|p| p == target.as_path())
                .unwrap_or(false);
            candidates.push(ModelCandidate {
                path: path_str,
                filename,
                size_mb,
                exists_in_target,
            });
        }
    }

    candidates
}

/// 将指定模型文件（及同级 config.json/tokenizer.json）复制到 ~/.铃记忆体/models/
#[tauri::command]
pub fn install_model(path: String) -> InstallModelResult {
    let src = PathBuf::from(&path);
    if !src.exists() {
        return InstallModelResult {
            success: false,
            message: format!("文件不存在：{path}"),
            missing_files: vec![],
        };
    }

    let target = target_models_dir();
    if let Err(e) = std::fs::create_dir_all(&target) {
        return InstallModelResult {
            success: false,
            message: format!("无法创建目录 {}：{e}", target.display()),
            missing_files: vec![],
        };
    }

    // safetensors 统一重命名为 model.safetensors，其余保留原名
    let src_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("model.safetensors");
    let dest_name = if src_name.to_lowercase().ends_with(".safetensors") {
        "model.safetensors".to_string()
    } else {
        src_name.to_string()
    };

    if let Err(e) = std::fs::copy(&src, target.join(&dest_name)) {
        return InstallModelResult {
            success: false,
            message: format!("复制模型文件失败：{e}"),
            missing_files: vec![],
        };
    }

    // 尝试复制同级 config.json / tokenizer.json
    let src_dir = src.parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    let mut missing = Vec::new();
    for companion in &["config.json", "tokenizer.json"] {
        let companion_src = src_dir.join(companion);
        if companion_src.exists() {
            let _ = std::fs::copy(&companion_src, target.join(companion));
        } else {
            missing.push((*companion).to_string());
        }
    }

    let message = if missing.is_empty() {
        format!("模型安装成功，完整文件已复制到 {}", target.display())
    } else {
        format!(
            "模型主文件已安装，但缺少辅助文件：{}，请手动补充后向量检索才能正常加载。",
            missing.join("、")
        )
    };

    InstallModelResult {
        success: true,
        message,
        missing_files: missing,
    }
}
