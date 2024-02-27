sc create kyber binPath="%~dp0service.exe" start=auto

netsh advfirewall firewall add rule name=KyberController dir=in action=allow program="%~dp0controller.exe" enable=yes profile=public
netsh advfirewall firewall add rule name=KyberAvServer dir=in action=allow program="%~dp0avserver.exe" enable=yes profile=public
netsh advfirewall firewall add rule name=KyberInputServer dir=in action=allow program="%~dp0inputserver.exe" enable=yes profile=public
