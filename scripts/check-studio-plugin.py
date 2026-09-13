from pathlib import Path
import sys

MAIN_PLUGIN = Path("plugins/roldex-studio/RoldexStudio.plugin.lua")
RUNTIME_PLUGIN = Path("plugins/roldex-studio/RoldexStudioRuntime.plugin.lua")
main = MAIN_PLUGIN.read_text(encoding="utf-8")
runtime = RUNTIME_PLUGIN.read_text(encoding="utf-8")

errors: list[str] = []

for label, text in (("main", main), ("runtime", runtime)):
    if "0.0.0.0" in text:
        errors.append(f"{label} Studio plugin must not use a non-loopback bridge")
    if "https://" in text:
        errors.append(f"{label} Studio plugin transport must remain local-only")
    if '"X-Roldex-Bridge"] = "studio"' not in text:
        errors.append(f"{label} Studio plugin is missing the bridge authentication header")
    if "http://127.0.0.1:38247" not in text:
        errors.append(f"{label} Studio plugin is missing the localhost default bridge URL")

for forbidden in ("sendButton.Enabled", "healthButton.Enabled", "applyButton.Enabled"):
    if forbidden in main:
        errors.append(f"GuiButton state must use Active, not {forbidden}")

main_required = (
    "ScriptEditorService:GetEditorSource(active)",
    "ScriptEditorService:UpdateSourceAsync",
    "ChangeHistoryService:TryBeginRecording",
    "ChangeHistoryService:FinishRecording",
    'Url = bridgeUrl() .. "/v1/actions"',
    'Url = bridgeUrl() .. "/v1/actions/result"',
    "local function executeMutationAction",
    "local function executeBatch",
    'op == "create"',
    'op == "set_properties"',
    'op == "update_script"',
    'op == "terrain_fill_block"',
    'command.action == "test"',
    "sendButton.Activated:Connect",
    "pollCommands()",
    "Enum.FinishRecordingOperation.Commit",
)

runtime_required = (
    "StudioCaptureService:CanCaptureScreenshot",
    "StudioCaptureService:RequestScreenshotPermissionAsync",
    "StudioCaptureService:CaptureScreenshot",
    "Enum.StudioCaptureScreenshotFormat.PNG",
    "EncodingService:Base64Encode",
    "StudioDeviceSimulatorService:GetDeviceListAsync",
    "StudioDeviceSimulatorService:SetDeviceAsync",
    "StudioDeviceSimulatorService:SetResolutionAsync",
    "StudioDeviceSimulatorService:SetOrientationAsync",
    "StudioDeviceSimulatorService:SetPixelDensityAsync",
    "StudioDeviceSimulatorService:SetScalingModeAsync",
    "UserInputService:CreateVirtualInput",
    "input:SendKey",
    "input:SendMouseButton",
    "input:SendPointerAction",
    "StudioTestService:ExecuteRunModeAsync",
    "StudioTestService:ExecutePlayModeAsync",
    "StudioTestService:ExecuteMultiplayerTestAsync",
    "StudioTestService:GetTestArgs()",
    "ScriptEditorService:UpdateSourceAsync",
    "StarterPlayerScripts",
    "__roldex_chat_messages",
    'kind == "chat"',
    "channel:SendAsync",
    "[RoldexChatTest][SENT]",
    "ReflectionService:GetPropertiesOfClass",
    'Url = bridgeUrl() .. "/v1/runtime-actions"',
    'Url = bridgeUrl() .. "/v1/actions/result"',
    'command.action == "capture"',
    'command.action == "scenario_test"',
    'command.action == "input"',
    'command.action == "device"',
    'command.action == "reflect"',
)

for marker in main_required:
    if marker not in main:
        errors.append(f"main Studio plugin missing expected marker: {marker}")

for marker in runtime_required:
    if marker not in runtime:
        errors.append(f"runtime Studio plugin missing expected marker: {marker}")

if 'value:match("^http://127%.0%.0%.1:%d+$")' not in main:
    errors.append("main Studio plugin must validate 127.0.0.1 loopback URLs")
if 'value:match("^http://127%.0%.0%.1:%d+$")' not in runtime:
    errors.append("runtime Studio plugin must validate 127.0.0.1 loopback URLs")

if "while true do" not in main or "pollCommands()" not in main:
    errors.append("main Studio plugin must continuously poll the live-build queue")
if "while true do" not in runtime or "pcall(poll)" not in runtime:
    errors.append("runtime Studio plugin must continuously poll the automation queue")

if "chatHarness:Destroy()" not in runtime:
    errors.append("runtime Studio plugin must clean up the temporary chat-test harness")

if errors:
    print("Roldex Studio static checks failed:", file=sys.stderr)
    for error in errors:
        print(f"- {error}", file=sys.stderr)
    raise SystemExit(1)

print("Roldex Studio main + runtime static checks passed")
