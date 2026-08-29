[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;
$cpu=Get-CimInstance Win32_Processor;
$ram=Get-CimInstance Win32_ComputerSystem;
$score_cpu=0; 
$sw=[System.Diagnostics.Stopwatch]::StartNew();
$s=0.0;for($i=0;$i -lt 5000000;$i++){$s+=[math]::Sqrt($i+1);}
$sw.Stop();
$cpuScore=[int](5000000/$sw.Elapsed.TotalSeconds);
$memGB=[math]::Round($ram.TotalPhysicalMemory/1GB,1);
Write-Output ("CPU：{0}（{1} 核）" -f $cpu.Name, $cpu.NumberOfCores);
Write-Output ("内存：{0} GB" -f $memGB);
Write-Output ("浮点得分：{0}" -f $cpuScore);
Write-Output ("综合评级：{0}" -f $(if($cpuScore -gt 8000000){'极速'}elseif($cpuScore -gt 4000000){'优秀'}elseif($cpuScore -gt 2000000){'良好'}else{'一般'}));
