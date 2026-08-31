// 《铃·记忆体》搜索引擎模式配置命令
use crate::config::store;
use crate::memory::search::check_vector_model;

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
