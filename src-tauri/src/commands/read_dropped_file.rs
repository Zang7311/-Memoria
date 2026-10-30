// 《铃·记忆体》把一个「被拖进窗口的文件」读成附件
//
// 前端从 Tauri 拖放事件里拿到的是**路径**，不是浏览器的 File 对象，所以内容得由这里读：
// 图片转 base64、文本转 UTF-8 字符串；扩展名白名单和大小上限跟前端「选择文件」那条路
// （src/utils/attachments.ts）保持一致，避免两条入口行为不一样。
use crate::error::AppError;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use std::path::Path;

/// 图片扩展名 → MIME
const IMAGE_EXT: [(&str, &str); 6] = [
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
];

/// 纯文本扩展名白名单（与前端保持一致）
const TEXT_EXT: [&str; 16] = [
    "txt", "md", "json", "csv", "log", "py", "js", "ts", "rs", "html", "css", "xml", "yaml", "toml",
    "ini", "sh",
];

const IMAGE_LIMIT: u64 = 5 * 1024 * 1024;
const TEXT_LIMIT: u64 = 1 * 1024 * 1024;

/// 拖进来读好的附件（字段名与前端 src/utils/attachments.ts 的 Attachment 对齐）
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DroppedAttachment {
    /// 文件名（只取名，不带路径）
    pub name: String,
    /// "image" | "text"
    pub kind: String,
    pub mime: String,
    /// 图片是 base64，文本是原文
    pub data: String,
    pub size: u64,
}

fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// 判断这个文件名/尺寸能不能当附件（纯逻辑，单测直接打这里）
fn classify(name: &str, size: u64) -> Result<(&'static str, &'static str), AppError> {
    let ext = extension_of(name);
    if let Some((_, mime)) = IMAGE_EXT.iter().find(|(e, _)| *e == ext) {
        if size > IMAGE_LIMIT {
            return Err(AppError::InternalError("图片附件不能超过 5MB".to_string()));
        }
        return Ok(("image", mime));
    }
    if TEXT_EXT.contains(&ext.as_str()) {
        if size > TEXT_LIMIT {
            return Err(AppError::InternalError("文本附件不能超过 1MB".to_string()));
        }
        return Ok(("text", "text/plain"));
    }
    Err(AppError::InternalError(format!(
        "暂不支持这种文件（.{ext}），支持图片和常见文本格式"
    )))
}

/// IPC：读取桌面拖进窗口的文件
#[tauri::command]
pub fn read_dropped_file(path: String) -> Result<DroppedAttachment, AppError> {
    let p = Path::new(&path);
    if !p.is_file() {
        return Err(AppError::InternalError("这个路径不是一个文件".to_string()));
    }
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "附件".to_string());
    let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
    let (kind, mime) = classify(&name, size)?;

    let bytes = std::fs::read(p).map_err(|e| AppError::InternalError(format!("读取失败：{e}")))?;
    let data = if kind == "image" {
        STANDARD.encode(&bytes)
    } else {
        String::from_utf8(bytes)
            .map_err(|_| AppError::InternalError("这个文本文件不是 UTF-8 编码，先转一下再拖进来".to_string()))?
    };

    Ok(DroppedAttachment {
        name,
        kind: kind.to_string(),
        mime: mime.to_string(),
        data,
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 图片按扩展名识别且有大小上限() {
        assert_eq!(classify("a.png", 100).unwrap(), ("image", "image/png"));
        assert_eq!(classify("截图.JPG", 100).unwrap(), ("image", "image/jpeg"));
        assert!(classify("big.png", IMAGE_LIMIT + 1).is_err());
    }

    #[test]
    fn 文本按白名单识别() {
        assert_eq!(classify("note.md", 10).unwrap(), ("text", "text/plain"));
        assert_eq!(classify("main.rs", 10).unwrap(), ("text", "text/plain"));
        assert!(classify("big.txt", TEXT_LIMIT + 1).is_err());
    }

    #[test]
    fn 不支持的类型给出人话提示() {
        let err = classify("setup.exe", 10).unwrap_err().to_string();
        assert!(err.contains("暂不支持"), "实际: {err}");
        let err2 = classify("noext", 10).unwrap_err().to_string();
        assert!(err2.contains("暂不支持"));
    }
}
