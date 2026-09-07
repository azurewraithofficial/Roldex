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
    "sendButton.Activated:Connect",
    "applyButton.Activated:Connect",
)

for marker in required:
    if marker not in text:
        errors.append(f"missing expected Studio safety/integration marker: {marker}")

if "0.0.0.0" in text:
    errors.append("Studio plugin must not default to a non-loopback bridge")

if errors:
    print("Roldex Studio static checks failed:", file=sys.stderr)
    for error in errors:
        print(f"- {error}", file=sys.stderr)
    raise SystemExit(1)

print("Roldex Studio static checks passed")
