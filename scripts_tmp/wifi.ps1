[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;
$profiles=@(netsh wlan show profiles | Select-String ':\s+(.+)$' | ForEach-Object {$_.Matches[0].Groups[1].Value.Trim()});
if($profiles.Count -eq 0){Write-Output '(未发现已保存的 WiFi 配置文件)';exit;}
foreach($n in $profiles){
  $k=(netsh wlan show profile name="$n" key=clear | Select-String '关键内容\s*:\s*(.+)' | ForEach-Object {$_.Matches[0].Groups[1].Value.Trim()});
  if(-not $k){$k=(netsh wlan show profile name="$n" key=clear | Select-String 'Key Content\s*:\s*(.+)' | ForEach-Object {$_.Matches[0].Groups[1].Value.Trim()});}
  Write-Output ("$n  :  " + $(if($k){$k}else{'(无/未保存密码)'}));
}
