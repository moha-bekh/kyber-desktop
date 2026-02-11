sc create kyber binPath="%~dp0service.exe" start=auto

netsh advfirewall firewall add rule name=KyberController dir=in action=allow program="%~dp0controller.exe" enable=yes profile=any
netsh advfirewall firewall add rule name=KyberAvServer dir=in action=allow program="%~dp0kyavserver.exe" enable=yes profile=any
netsh advfirewall firewall add rule name=KyberInputServer dir=in action=allow program="%~dp0kynputserver.exe" enable=yes profile=any
