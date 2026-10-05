use crate::agent::goals::{self, GoalReport, GoalSnapshot};
use crate::error::AppError;
use crate::types::{AgentGoal, GoalRunBudget, GoalStep};
use tauri::{AppHandle, Emitter};

fn changed(app: &AppHandle) {
    goals::wake_timer();
    if let Ok(snapshot) = goals::store().and_then(|store| store.snapshot()) {
        let _ = app.emit("goals_changed", snapshot);
    }
}

#[tauri::command]
pub fn list_goals() -> Result<GoalSnapshot, AppError> {
    goals::store()?.snapshot()
}

#[tauri::command]
pub fn create_goal(
    app: AppHandle,
    title: String,
    description: String,
    steps: Option<Vec<GoalStep>>,
    budget: Option<GoalRunBudget>,
) -> Result<AgentGoal, AppError> {
    let goal = goals::store()?.create(
        title,
        description,
        steps.unwrap_or_default(),
        budget.unwrap_or_default(),
    )?;
    changed(&app);
    Ok(goal)
}

#[tauri::command]
pub async fn goal_advance(app: AppHandle, id: String) -> Result<GoalReport, AppError> {
    goals::advance(&app, &id).await
}

#[tauri::command]
pub fn set_goal_status(app: AppHandle, id: String, status: String) -> Result<AgentGoal, AppError> {
    let goal = goals::store()?.set_status(&id, &status)?;
    changed(&app);
    Ok(goal)
}

#[tauri::command]
pub fn update_goal_settings(
    app: AppHandle,
    id: String,
    auto_advance: bool,
    auto_interval_secs: u32,
    budget: GoalRunBudget,
) -> Result<AgentGoal, AppError> {
    let goal = goals::store()?.settings(&id, auto_advance, auto_interval_secs, budget)?;
    changed(&app);
    Ok(goal)
}

#[tauri::command]
pub fn delete_goal(app: AppHandle, id: String) -> Result<(), AppError> {
    goals::store()?.delete(&id)?;
    changed(&app);
    Ok(())
}
