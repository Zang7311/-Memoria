use crate::error::AppError;
use crate::types::Attachment;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};

pub const IMAGE_LIMIT: usize = 5 * 1024 * 1024;
pub const TEXT_LIMIT: usize = 1024 * 1024;
pub const MERGED_TEXT_LIMIT: usize = 200 * 1024;

pub fn image_mime(name: &str) -> Option<&'static str> {
    match name.rsplit_once('.')?.1.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

pub fn validate(attachments: &[Attachment]) -> Result<(), AppError> {
    for attachment in attachments {
        let invalid = |message: &str| AppError::ConfigError(message.to_string());
        if attachment.name.is_empty()
            || attachment.name.len() > 1024
            || attachment.name.chars().any(char::is_control)
        {
            return Err(invalid("附件文件名无效"));
        }
        let actual_size = match attachment.kind.as_str() {
            "image" => {
                let Some(mime) = image_mime(&attachment.name) else {
                    return Err(invalid("暂不支持这种文件"));
                };
                if attachment.size > IMAGE_LIMIT
                    || attachment.data.len() > 4 * IMAGE_LIMIT.div_ceil(3)
                {
                    return Err(invalid("图片附件不能超过 5MB"));
                }
                if !attachment.mime.is_empty() && attachment.mime != mime {
                    return Err(invalid("图片附件类型与文件后缀不一致"));
                }
                let bytes = STANDARD
                    .decode(&attachment.data)
                    .map_err(|_| invalid("图片附件编码无效"))?;
                if bytes.len() > IMAGE_LIMIT {
                    return Err(invalid("图片附件不能超过 5MB"));
                }
                bytes.len()
            }
            "text" => {
                let extension = attachment
                    .name
                    .rsplit_once('.')
                    .map(|(_, extension)| extension)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if ![
                    "txt", "md", "json", "csv", "log", "py", "js", "ts", "rs", "html", "css",
                    "xml", "yaml", "toml", "ini", "sh",
                ]
                .contains(&extension.as_str())
                {
                    return Err(invalid("暂不支持这种文件"));
                }
                if attachment.size > TEXT_LIMIT || attachment.data.len() > TEXT_LIMIT {
                    return Err(invalid("文本附件不能超过 1MB"));
                }
                attachment.data.len()
            }
            _ => return Err(invalid("暂不支持这种文件")),
        };
        if attachment.kind == "image" && attachment.size != 0 && attachment.size != actual_size {
            return Err(invalid("附件大小与实际内容不一致"));
        }
    }
    Ok(())
}

pub fn has_image(attachments: &[Attachment]) -> bool {
    attachments
        .iter()
        .any(|attachment| attachment.kind == "image")
}

pub fn merge_text(input: &str, attachments: &[Attachment]) -> String {
    if attachments.is_empty() {
        return input.to_string();
    }
    let mut appended = input.to_string();
    let marker = "\n【已截断】";
    if appended.len() > MERGED_TEXT_LIMIT {
        let mut end = MERGED_TEXT_LIMIT - marker.len();
        while !appended.is_char_boundary(end) {
            end -= 1;
        }
        appended.truncate(end);
        appended.push_str(marker);
        return appended;
    }
    for attachment in attachments
        .iter()
        .filter(|attachment| attachment.kind == "text")
    {
        let section = format!("\n\n【附件 {}】\n{}", attachment.name, attachment.data);
        if appended.len() + section.len() <= MERGED_TEXT_LIMIT {
            appended.push_str(&section);
        } else {
            let mut end = MERGED_TEXT_LIMIT.saturating_sub(marker.len());
            if appended.len() > end {
                while !appended.is_char_boundary(end) {
                    end -= 1;
                }
                appended.truncate(end);
            } else {
                end -= appended.len();
                while !section.is_char_boundary(end) {
                    end -= 1;
                }
                appended.push_str(&section[..end]);
            }
            appended.push_str(marker);
            break;
        }
    }
    appended
}

pub fn user_content(input: &str, attachments: &[Attachment]) -> Value {
    let text = merge_text(input, attachments);
    if !has_image(attachments) {
        return json!(text);
    }
    let mut parts = vec![json!({ "type": "text", "text": text })];
    for attachment in attachments
        .iter()
        .filter(|attachment| attachment.kind == "image")
    {
        let mime = image_mime(&attachment.name).unwrap_or("image/jpeg");
        parts.push(json!({ "type": "image_url", "image_url": {
            "url": format!("data:{mime};base64,{}", attachment.data)
        }}));
    }
    json!(parts)
}

pub fn classification_input(input: &str, attachments: &[Attachment]) -> String {
    let mut context = input.to_string();
    for attachment in attachments {
        let label = if attachment.kind == "image" {
            "图片"
        } else {
            "文本"
        };
        let size = if attachment.size == 0 && attachment.kind == "text" {
            attachment.data.len()
        } else {
            attachment.size
        };
        context.push_str(&format!(
            "\n[附件: {}, {}字节, {label}]",
            attachment.name, size
        ));
    }
    context
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::types::{AttachmentMeta, Message};

    pub fn text(name: &str, data: &str) -> Attachment {
        Attachment {
            kind: "text".into(),
            name: name.into(),
            mime: "text/plain".into(),
            data: data.into(),
            size: data.len(),
        }
    }

    pub fn image() -> Attachment {
        Attachment {
            kind: "image".into(),
            name: "photo.JPG".into(),
            mime: "image/jpeg".into(),
            data: STANDARD.encode([1, 2, 3]),
            size: 3,
        }
    }

    #[test]
    fn 无附件与文本附件保持字符串格式() {
        assert_eq!(user_content("在吗", &[]), json!("在吗"));
        assert_eq!(
            user_content("你好", &[text("a.txt", "第一份"), text("b.rs", "第二份")]),
            json!("你好\n\n【附件 a.txt】\n第一份\n\n【附件 b.rs】\n第二份")
        );
    }

    #[test]
    fn 图片与文本附件构造多模态数组() {
        let content = user_content("看一下", &[image(), text("a.txt", "说明"), image()]);
        assert_eq!(content.as_array().unwrap().len(), 3);
        assert_eq!(content[0]["text"], "看一下\n\n【附件 a.txt】\n说明");
        assert_eq!(
            content[1]["image_url"]["url"],
            "data:image/jpeg;base64,AQID"
        );
    }

    #[test]
    fn 文本总量受限且中文安全截断() {
        let content = merge_text(
            "用户文字",
            &[
                text("a.txt", &"中".repeat(100000)),
                text("b.txt", "不会追加"),
            ],
        );
        assert!(content.ends_with("【已截断】"));
        assert!(content.len() <= MERGED_TEXT_LIMIT);
        assert!(!content.contains("不会追加"));
        let exact = "a".repeat(MERGED_TEXT_LIMIT - "\n\n【附件 a.txt】\n".len());
        let content = merge_text("", &[text("a.txt", &exact), text("b.txt", "中")]);
        assert!(content.len() <= MERGED_TEXT_LIMIT);
        assert!(content.ends_with("【已截断】"));
    }

    #[test]
    fn 附件超限不能通过虚报大小绕过() {
        let mut attachment = image();
        attachment.data = STANDARD.encode(vec![0; IMAGE_LIMIT + 1]);
        attachment.size = 0;
        assert!(validate(&[attachment])
            .unwrap_err()
            .to_string()
            .contains("图片附件不能超过 5MB"));
        let mut attachment = text("a.txt", &"a".repeat(TEXT_LIMIT + 1));
        attachment.size = 0;
        assert!(validate(&[attachment])
            .unwrap_err()
            .to_string()
            .contains("文本附件不能超过 1MB"));
        let mut attachment = image();
        attachment.size = IMAGE_LIMIT + 1;
        assert!(validate(&[attachment])
            .unwrap_err()
            .to_string()
            .contains("5MB"));
        let mut attachment = text("a.txt", "小文件");
        attachment.size = TEXT_LIMIT + 1;
        assert!(validate(&[attachment])
            .unwrap_err()
            .to_string()
            .contains("1MB"));
    }

    #[test]
    fn 限制边界与省略字段兼容() {
        let mut attachment = image();
        attachment.data = STANDARD.encode(vec![0; IMAGE_LIMIT]);
        attachment.size = IMAGE_LIMIT;
        validate(&[attachment, text("a.txt", &"a".repeat(TEXT_LIMIT))]).unwrap();
        let attachment: Attachment =
            serde_json::from_value(json!({"kind":"text","name":"a.txt","data":"内容"})).unwrap();
        validate(&[attachment]).unwrap();
        validate(&[]).unwrap();
    }

    #[test]
    fn 非法类型编码与大小返回中文且不泄漏内容() {
        for attachment in [
            text("a.pdf", "secret"),
            text("txt", "secret"),
            text("a.png", "secret"),
            Attachment {
                name: "jpg".into(),
                ..image()
            },
            Attachment {
                data: "secret!".into(),
                ..image()
            },
            Attachment {
                mime: "text/plain".into(),
                ..image()
            },
            Attachment { size: 9, ..image() },
        ] {
            let error = validate(&[attachment]).unwrap_err().to_string();
            assert!(!error.contains("secret"));
            assert!(error.chars().any(|character| character >= '\u{4e00}'));
        }
    }

    #[test]
    fn 判断上下文只有元信息() {
        let context = classification_input("你好", &[image(), text("a.txt", "secret")]);
        assert!(context.contains("photo.JPG, 3字节, 图片"));
        assert!(context.contains("a.txt, 6字节, 文本"));
        assert!(!context.contains("secret"));
        assert!(!context.contains("AQID"));
        assert_eq!(classification_input("你好", &[]), "你好");
    }

    #[test]
    fn 会话只保存元信息并重放占位且兼容旧历史() {
        let mut message: Message = serde_json::from_value(
            json!({"id":"1","role":"user","content":"你好","timestamp":"now"}),
        )
        .unwrap();
        assert!(!serde_json::to_value(&message)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("attachments"));
        message.attachments = vec![
            AttachmentMeta {
                kind: "image".into(),
                name: "photo.jpg".into(),
            },
            AttachmentMeta {
                kind: "text".into(),
                name: "report.txt".into(),
            },
        ];
        let stored = serde_json::to_string(&message).unwrap();
        assert!(!stored.contains("data"));
        let loaded: Message = serde_json::from_str(&stored).unwrap();
        assert_eq!(
            loaded.replay_content(),
            "你好\n[图片: photo.jpg]\n[附件: report.txt]"
        );
    }

    #[test]
    fn 附件请求总文本包含用户输入上限且无附件不截断() {
        let input = "字".repeat(MERGED_TEXT_LIMIT);
        assert_eq!(merge_text(&input, &[]), input);
        let merged = merge_text(&input, &[text("a.txt", "附件内容")]);
        assert!(merged.len() <= MERGED_TEXT_LIMIT);
        assert!(merged.ends_with("【已截断】"));
        let merged = merge_text(&input, &[image()]);
        assert!(merged.len() <= MERGED_TEXT_LIMIT);
        assert!(merged.ends_with("【已截断】"));
    }
}
