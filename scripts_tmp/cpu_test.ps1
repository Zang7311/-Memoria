[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;
$sw=[System.Diagnostics.Stopwatch]::StartNew();
$sum=0.0;
for($i=0;$i -lt 3000000;$i++){$sum+=[math]::Sqrt($i+1)*[math]::Cos($i*0.0001);}
$sw.Stop();
Write-Output ("CPU 浮点测试：300 万次 sqrt+cos 运算耗时 {0:N2} 秒" -f $sw.Elapsed.TotalSeconds);
Write-Output ("得分：{0:N0}（越高越快）" -f (3000000/$sw.Elapsed.TotalSeconds));
