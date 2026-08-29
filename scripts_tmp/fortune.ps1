[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;
$r=Get-Random -Minimum 0 -Maximum 101;
if($r -ge 90){$msg='大吉！今日诸事顺遂，宜大胆行动～';}
elseif($r -ge 70){$msg='吉！宜出行、交友，把握良机。';}
elseif($r -ge 50){$msg='中吉～稳中有进，别急别慌。';}
elseif($r -ge 30){$msg='小凶：宜静不宜动，多歇歇。';}
else{$msg='大凶！注意避坑，但别怕，铃会陪着你～';}
Write-Output "今日运势：$r / 100"
Write-Output $msg
