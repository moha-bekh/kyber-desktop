sc delete kyber

netsh advfirewall firewall delete rule name=KyberController
netsh advfirewall firewall delete rule name=KyberTxproto
netsh advfirewall firewall delete rule name=KyberInputServer
