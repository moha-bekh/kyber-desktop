sc create kyber binPath=%~dp0\service.exe

netsh advfirewall firewall add rule name=KyberController dir=in action=allow program=%~dp0\controller.exe enable=yes profile=public,private
netsh advfirewall firewall add rule name=KyberTxproto dir=in action=allow program=%~dp0\txproto.exe enable=yes profile=public,private
netsh advfirewall firewall add rule name=KyberInputServer dir=in action=allow program=%~dp0\input_server.exe enable=yes profile=public,private
