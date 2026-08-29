// 《铃·记忆体》OCR 文字识别命令（moon11）
// 双引擎切换：优先 Windows 内置 OCR，失败则降级到 Tesseract
use crate::error::AppError;
use std::path::Path;
use std::process::Stdio;

#[derive(serde::Serialize, Clone)]
pub struct OcrResult {
    pub engine: String,  // "windows" 或 "tesseract"
    pub text: String,
    pub success: bool,
}

/// 尝试 Windows 内置 OCR 识别
fn try_windows_ocr(image_path: &str) -> Result<String, String> {
    // 这里复用现有的 PowerShell 脚本逻辑
    // 由于 Windows OCR 需要复杂的 WinRT API 调用，
    // 我们暂时保留现有的 PowerShell 脚本方式
    let output = std::process::Command::new("powershell")
        .args(&["-NoProfile", "-EncodedCommand"])
        .arg(build_windows_ocr_ps_command(image_path))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("PowerShell 执行失败: {e}"))?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        // 检查是否是 OCR engine not available 错误
        if stdout.contains("OCR engine not available") {
            Err("Windows OCR 不可用（需要语言包）".into())
        } else {
            Ok(stdout)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("Windows OCR 失败: {}", stderr))
    }
}

/// 构建 Windows OCR PowerShell 命令（Base64 编码）
fn build_windows_ocr_ps_command(image_path: &str) -> String {
    // 这是从 toolbox_presets.json 中提取的 Windows OCR PowerShell 脚本
    // 我们只编码图像路径参数
    use base64::Engine;
    let ps_script = format!(r#"
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$null = [Windows.Storage.StorageFile,Windows.Storage,ContentType=WindowsRuntime]
$null = [Windows.Media.Ocr.OcrEngine,Windows.Foundation,ContentType=WindowsRuntime]
$null = [Windows.Graphics.Imaging.BitmapDecoder,Windows.Graphics,ContentType=WindowsRuntime]

$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {{ $_.Name -eq 'AsTask' -and $_.IsGenericMethodDefinition }})[0]
function Await($WinRtTask, $ResultType) {{
    $asTask = $asTaskGeneric.MakeGenericMethod($ResultType)
    $netTask = $asTask.Invoke($null, @($WinRtTask))
    $netTask.Wait()
    $netTask.Result
}}

$path = '{}'
if (-not (Test-Path -LiteralPath $path)) {{
    Write-Output "图片文件不存在: $path"
    exit 1
}}

try {{
    $file = [Windows.Storage.StorageFile]::GetFileFromPathAsync($path)
    $storageFile = Await $file ([Windows.Storage.StorageFile])
    $stream = $storageFile.OpenAsync([Windows.Storage.FileAccessMode]::Read)
    $randomAccessStream = Await $stream ([Windows.Storage.Streams.IRandomAccessStream])
    $decoder = [Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($randomAccessStream)
    $bitmapDecoder = Await $decoder ([Windows.Graphics.Imaging.BitmapDecoder])
    $bitmap = $bitmapDecoder.GetSoftwareBitmapAsync()
    $softwareBitmap = Await $bitmap ([Windows.Graphics.Imaging.SoftwareBitmap])
    
    if ($softwareBitmap.BitmapPixelFormat -ne [Windows.Graphics.Imaging.BitmapPixelFormat]::Bgra8) {{
        $converted = [Windows.Graphics.Imaging.SoftwareBitmap]::Convert($softwareBitmap, [Windows.Graphics.Imaging.BitmapPixelFormat]::Bgra8)
        $softwareBitmap.Dispose()
        $softwareBitmap = $converted
    }}
    
    $ocrEngine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
    if (-not $ocrEngine) {{
        Write-Output "OCR engine not available (need language pack)"
        exit 1
    }}
    
    $ocrResultTask = $ocrEngine.RecognizeAsync($softwareBitmap)
    $ocrResult = Await $ocrResultTask ([Windows.Media.Ocr.OcrResult])
    $softwareBitmap.Dispose()
    
    Write-Output $ocrResult.Text
}} catch {{
    Write-Output "OCR 识别失败: $_"
    exit 1
}}
"#, image_path.replace("'", "''"));

    base64::engine::general_purpose::STANDARD.encode(ps_script.as_bytes())
}

/// 尝试 Tesseract OCR 识别
fn try_tesseract_ocr(image_path: &str) -> Result<String, String> {
    // 检查 tesseract 是否可用
    let tesseract_exists = std::process::Command::new("where")
        .arg("tesseract")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !tesseract_exists {
        return Err("Tesseract 未安装，请运行: winget install UB-Mannheim.TesseractOCR".into());
    }

    // 创建临时文件用于输出
    let temp_dir = std::env::temp_dir();
    let output_file = temp_dir.join("tesseract_output.txt");
    
    let output = std::process::Command::new("tesseract")
        .arg(image_path)
        .arg(output_file.to_str().unwrap())
        .arg("-l")
        .arg("eng+chi_sim")  // 英语 + 简体中文
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Tesseract 执行失败: {e}"))?;

    if output.status.success() {
        // Tesseract 会生成 output_file.txt
        let result_path = format!("{}.txt", output_file.to_str().unwrap());
        match std::fs::read_to_string(&result_path) {
            Ok(text) => {
                // 清理临时文件
                let _ = std::fs::remove_file(&result_path);
                Ok(text.trim().to_string())
            }
            Err(e) => Err(format!("读取 Tesseract 输出失败: {e}")),
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(format!("Tesseract 识别失败: {}", stderr))
    }
}

/// OCR 图像识别主函数：优先 Windows OCR，失败则降级到 Tesseract
#[tauri::command]
pub fn ocr_image(image_path: String) -> Result<OcrResult, AppError> {
    if !Path::new(&image_path).exists() {
        return Err(AppError::InternalError(format!("图片文件不存在: {}", image_path)));
    }

    // 首先尝试 Windows OCR
    match try_windows_ocr(&image_path) {
        Ok(text) => {
            return Ok(OcrResult {
                engine: "windows".into(),
                text,
                success: true,
            });
        }
        Err(windows_err) => {
            log::warn!("Windows OCR 失败: {}", windows_err);
            // Windows OCR 失败，尝试 Tesseract
            match try_tesseract_ocr(&image_path) {
                Ok(text) => {
                    return Ok(OcrResult {
                        engine: "tesseract".into(),
                        text,
                        success: true,
                    });
                }
                Err(tesseract_err) => {
                    log::error!("Tesseract OCR 也失败: {}", tesseract_err);
                    // 两个引擎都失败
                    Err(AppError::InternalError(format!(
                        "OCR 识别失败：\n1. Windows OCR: {}\n2. Tesseract: {}",
                        windows_err, tesseract_err
                    )))
                }
            }
        }
    }
}