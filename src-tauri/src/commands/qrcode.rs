// 《铃·记忆体》二维码生成与识别命令（moon10）
// 纯 Rust 实现，离线可用，无系统依赖：
//   qrcode crate —— 生成二维码
//   image  crate —— PNG 编码 / 图片解码
//   rqrr   crate —— 二维码解码识别
use crate::error::AppError;
use std::path::Path;

/// 生成二维码 PNG，保存到桌面，返回保存路径
/// text: 要编码进二维码的内容；size: 每模块像素大小（默认 10，可留空）
#[tauri::command]
pub fn generate_qrcode(text: String, size: Option<u32>) -> Result<String, AppError> {
    if text.trim().is_empty() {
        return Err(AppError::InternalError("二维码内容不能为空".into()));
    }
    // 生成二维码矩阵（自动选择合适的版本与纠错等级）
    let code = qrcode::QrCode::new(text.as_bytes())
        .map_err(|e| AppError::InternalError(format!("二维码生成失败：{e}")))?;
    // 取出按行主序排列的模块（true=深色模块），不含静区
    let matrix = code.to_vec();
    let n = code.width() as usize;
    // 手动渲染为灰度 PNG（image 0.25），带 4 模块静区便于扫描
    let scale = size.unwrap_or(10).clamp(2, 64) as u32;
    let quiet = 4u32;
    let dim = (n as u32 + quiet * 2) * scale;
    let mut img = image::ImageBuffer::new(dim, dim);
    for y in 0..dim {
        for x in 0..dim {
            let mx = (x / scale) as i64 - quiet as i64;
            let my = (y / scale) as i64 - quiet as i64;
            let dark = mx >= 0
                && my >= 0
                && (mx as usize) < n
                && (my as usize) < n
                && matrix[(my as usize) * n + (mx as usize)];
            let v = if dark { 0u8 } else { 255u8 };
            img.put_pixel(x, y, image::Luma([v]));
        }
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    // 保存到桌面（参考 save_pixel_art 的保存方式）
    let path = std::env::var("USERPROFILE")
        .map(|p| format!("{}\\Desktop\\qrcode_{}.png", p, ts))
        .unwrap_or_else(|_| format!("qrcode_{}.png", ts));
    img.save(&path)
        .map_err(|e| AppError::InternalError(format!("二维码图片保存失败：{e}")))?;
    Ok(path)
}

/// 识别图片中的二维码，返回解码出的文本内容
/// image_path: 图片完整路径（PNG/JPG 等）
#[tauri::command]
pub fn decode_qrcode(image_path: String) -> Result<String, AppError> {
    let path = Path::new(&image_path);
    if !path.exists() {
        return Err(AppError::InternalError(format!("图片不存在：{image_path}")));
    }
    // 读取图片并转为灰度
    let img = image::open(path)
        .map_err(|e| AppError::InternalError(format!("读取图片失败：{e}")))?
        .to_luma8();
    let mut prepared = rqrr::PreparedImage::prepare(img);
    let grids = prepared.detect_grids();
    if grids.is_empty() {
        return Err(AppError::InternalError(
            "未在图片中检测到二维码，请确认图片清晰、包含完整的二维码".into(),
        ));
    }
    // 逐个尝试解码（图片里可能有多个二维码）
    let mut last_err = None;
    for mut grid in grids {
        match grid.decode() {
            Ok((_, content)) => return Ok(content),
            Err(e) => last_err = Some(e),
        }
    }
    Err(AppError::InternalError(format!(
        "二维码解码失败（可能图片模糊或损坏）：{:?}",
        last_err
    )))
}
