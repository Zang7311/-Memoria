// 《铃·记忆体》AI-5 插件命令：执行插件技能
// 自然语言触发入口（如"帮我找一下昨天的文件" → file_search 技能）
use crate::types::{ExecuteSkillRequest, ExecuteSkillResponse};

/// 执行指定技能，返回结果（成功/失败均在响应中，不抛错）
#[tauri::command]
pub async fn execute_skill(req: ExecuteSkillRequest) -> ExecuteSkillResponse {
    // 先在管理器锁内取出插件+技能+授权（clone 后释放锁，避免持锁 await）
    let found = crate::plugin::with_manager(|m| m.find_enabled_skill(&req.skill_name));
    let (plugin, skill, granted) = match found {
        Ok(x) => x,
        Err(e) => {
            return ExecuteSkillResponse {
                success: false,
                result: None,
                error: Some(e.to_string()),
            }
        }
    };
    crate::plugin::runner::execute_skill(&plugin, &skill, &granted, req.params).await
}
