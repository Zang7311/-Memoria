[Console]::OutputEncoding=[System.Text.Encoding]::UTF8;
$ips=Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object {$_.IPAddress -ne '127.0.0.1'} | Select-Object -ExpandProperty IPAddress;
if(-not $ips){$ips='(未获取到局域网 IP)';}
Write-Output "内网 IPv4：$($ips -join ', ')"
