[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;
Add-Type -AssemblyName System.Windows.Forms,System.Drawing;
$b=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds;
$bmp=New-Object System.Drawing.Bitmap($b.Width,$b.Height);
$g=[System.Drawing.Graphics]::FromImage($bmp);
$g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size);
$p=Join-Path ([Environment]::GetFolderPath('Desktop')) ('screenshot_'+[DateTime]::Now.ToString('yyyyMMdd_HHmmss')+'.png');
$bmp.Save($p,[System.Drawing.Imaging.ImageFormat]::Png);
Write-Output "Saved: $p"
