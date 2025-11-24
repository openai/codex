:: environment setting for dbus clients
@echo off

:: session bus address
set DBUS_SESSION_BUS_ADDRESS=@DBUS_SESSION_BUS_CONNECT_ADDRESS@

:: system bus address
set DBUS_SYSTEM_BUS_DEFAULT_ADDRESS=@DBUS_SYSTEM_BUS_DEFAULT_ADDRESS@
