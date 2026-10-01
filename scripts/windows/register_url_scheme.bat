@echo off
rem Register the kyber:// URL scheme for the current user so clicking a portal
rem link launches the client. Per-user (HKCU), no administrator rights needed.
rem Run once after extracting the package. %~dp0 is this script's directory;
rem the package places kyclient.exe alongside it.

reg add "HKCU\Software\Classes\kyber" /ve /d "URL:Kyber Protocol" /f || goto :fail
reg add "HKCU\Software\Classes\kyber" /v "URL Protocol" /d "" /f || goto :fail
reg add "HKCU\Software\Classes\kyber\DefaultIcon" /ve /d "\"%~dp0kyclient.exe\",0" /f || goto :fail
reg add "HKCU\Software\Classes\kyber\shell\open\command" /ve /d "\"%~dp0kyclient.exe\" \"%%1\"" /f || goto :fail

echo Registered kyber:// scheme handler.
exit /b 0

:fail
echo Failed to register the kyber:// scheme handler. 1>&2
exit /b 1
