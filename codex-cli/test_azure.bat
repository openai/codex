@echo off
cd /d C:\codex\codex-cli

rem Set environment variables
set AZURE_API_KEY=Your_API_Key_Here
set AZURE_ENDPOINT=Your_Endpoint_URL_Here
set AZURE_ADDITIONAL_HEADERS=Your_Additional_Headers_Here_If_Applicable

rem Verify they are set
echo Environment variables set:
echo AZURE_API_KEY=%AZURE_API_KEY%
echo AZURE_ENDPOINT=%AZURE_ENDPOINT%
echo AZURE_ENDPOINT=%AZURE_ADDITIONAL_HEADERS%

rem .\test_azure.bat "Dear codex, what is 2+2?"
echo.
echo Starting Codex CLI with Azure OpenAI o3 deployment...
node .\dist\cli.js --provider azure --model o3 %*
