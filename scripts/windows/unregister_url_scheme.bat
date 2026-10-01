@echo off
rem Remove the kyber:// URL scheme registration for the current user.

reg delete "HKCU\Software\Classes\kyber" /f || goto :fail

echo Unregistered kyber:// scheme handler.
exit /b 0

:fail
echo Failed to unregister the kyber:// scheme handler. 1>&2
exit /b 1
