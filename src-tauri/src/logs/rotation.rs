// 《铃·记忆体》日志轮转（AI-7 任务 6）
// 单文件超 10MB 时：app.log → app.1.log → app.2.log ... 保留 max_backups 个。
// 并额外裁剪最旧备份，保证日志目录总大小 ≤ 50MB（任务书注意事项 5）。
use std::path::Path;

/// 执行轮转（假设调用方已关闭主文件句柄）
/// - 删除最旧备份 app.{max}.log
/// - 依次下移：app.1 → app.2, ..., app.{max-1} → app.{max}
/// - 主文件 app.log → app.1.log
pub fn rotate(app_log: &Path, max_backups: usize) {
    let dir = match app_log.parent() {
        Some(d) => d.to_path_buf(),
        None => return,
    };

    // 删除最旧备份
    let last = dir.join(format!("app.{max_backups}.log"));
    let _ = std::fs::remove_file(&last);

    // 高→低下移
    for i in (1..max_backups).rev() {
        let from = dir.join(format!("app.{i}.log"));
        let to = dir.join(format!("app.{}.log", i + 1));
        if from.exists() {
            let _ = std::fs::rename(&from, &to);
        }
    }

    // 主文件 → app.1.log
    let first = dir.join("app.1.log");
    if app_log.exists() {
        let _ = std::fs::rename(app_log, &first);
    }

    // 总大小裁剪：若全部备份 + 新主文件总大小仍超 50MB，删除最旧备份直到达标
    enforce_total_limit(&dir, 50 * 1024 * 1024);
}

/// 控制日志目录总大小 ≤ 50MB（删除最旧备份）
fn enforce_total_limit(dir: &Path, limit: u64) {
    // 收集所有 app.N.log
    let mut files: Vec<(String, u64)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("app.") && name.ends_with(".log") {
                if let Ok(meta) = e.metadata() {
                    files.push((name, meta.len()));
                }
            }
        }
    }
    // 按序号从大到小排（最旧 = 序号最大）
    files.sort_by(|a, b| {
        let na = a.0.trim_start_matches("app.").trim_end_matches(".log").parse::<u64>().unwrap_or(0);
        let nb = b.0.trim_start_matches("app.").trim_end_matches(".log").parse::<u64>().unwrap_or(0);
        nb.cmp(&na) // 大到小
    });
    let mut total: u64 = files.iter().map(|(_, s)| *s).sum();
    for (name, size) in files {
        if total <= limit {
            break;
        }
        let _ = std::fs::remove_file(dir.join(&name));
        total = total.saturating_sub(size);
    }
}
