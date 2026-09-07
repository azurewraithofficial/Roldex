from pathlib import Path
import sys

PLUGIN = Path("plugins/roldex-studio/RoldexStudio.plugin.lua")
text = PLUGIN.read_text(encoding="utf-8")

errors: list[str] = []

for forbidden in (
    "sendButton.Enabled",
    "applyButton.Enabled",
    "healthButton.Enabled",
):
    if forbidden in text:
        errors.append(f"GuiButton state must use Active, not {forbidden}")

required = (
    'local DEFAULT_BRIDGE_URL = "http://127.0.0.1:38247"',
    'options.Headers["X-Roldex-Bridge"] = "studio"',
    "ScriptEditorService:GetEditorSource(active)",
    "ScriptEditorService:UpdateSourceAsync(active",
    "ScriptEditorService:UpdateSourceAsync(script",
    "ChangeHistoryService:TryBeginRecording",
    "ChangeHistoryService:FinishRecording",
    "StudioTestService:ExecuteRunModeAsync",
    "StudioTestService:ExecutePlayModeAsync",
    "StudioTestService:ExecuteMultiplayerTestAsync",
    'Url = bridgeUrl() .. "/v1/actions"',
    'Url = bridgeUrl() .. "/v1/actions/result"',
    "local function executeMutationAction",
    'op == "create"',
    'op == "set_properties"',
    'op == "update_script"',
    'op == "terrain_fill_block"',
    "sendButton.Activated:Connect",
    "applyButton.Activated:Connect",
)

for marker in required:
    if marker not in text:
        errors.append(f"missing expected Studio safety/integration marker: {marker}")

if "0.0.0.0" in text:
    errors.append("Studio plugin must not default to a non-loopback bridge")

if "https://" in text:
    errors.append("Studio plugin bridge transport must remain local-only; unexpected https:// found")

if 'value:match("^http://127%.0%.0%.1:%d+$")' not in text:
    errors.append("Studio bridge URL must validate 127.0.0.1 loopback URLs")

if "while true do" not in text or "pollCommands()" not in text:
    errors.append("Studio plugin must continuously poll the local Roldex action queue")

if errors:
    print("Roldex Studio static checks failed:", file=sys.stderr)
    for error in errors:
        print(f"- {error}", file=sys.stderr)
    raise SystemExit(1)

print("Roldex Studio static checks passed")
