:: bus-test wrapper
@echo off

:: session bus address
set DBUS_STARTER_BUS=tcp:host=localhost,port=1234

if NOT "%1" == "" (
	SET DATADIR=%1
) else (
	SET DATADIR=test\data
)

bin\bus-test.exe test\data

