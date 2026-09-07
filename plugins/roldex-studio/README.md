# Roldex Studio plugin

Roldex Studio is the companion Roblox Studio plugin for the Roldex CLI.

## What it does

- opens a docked Roldex chat panel inside Studio
- connects only to the local Roldex CLI bridge on `127.0.0.1` by default
- sends the current Studio selection to Roldex
- sends the active Script/LocalScript/ModuleScript editor source using `ScriptEditorService:GetEditorSource()`
- shows the Roldex response in Studio
- lets you explicitly apply the first returned Luau code block to the active script with `ScriptEditorService:UpdateSourceAsync()`

Roldex never auto-applies a model response from the plugin. Applying code requires clicking **Apply First Luau Block to Active Script**.

## Install

### Windows

The repository installer installs the plugin by default:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\install.ps1
```

The plugin file is copied to:

```text
%LOCALAPPDATA%\Roblox\Plugins\RoldexStudio.plugin.lua
```

### macOS

Run:

```bash
./scripts/install.sh
```

The plugin file is copied to:

```text
~/Documents/Roblox/Plugins/RoldexStudio.plugin.lua
```

### Manual

In Roblox Studio, use **Plugins → Plugins Folder** to open the exact local plugins directory, then copy `RoldexStudio.plugin.lua` there and restart Studio.

## Use

1. Start Roldex from your Roblox/Rojo project directory:

   ```text
   roldex
   ```

2. Open Roblox Studio and click **Roldex** in the Plugins toolbar.
3. The first HTTP request may make Studio ask for permission for the plugin to communicate with the local address. Allow it.
4. Select instances or open a script, type a request, and click **Send with Studio Context**.
5. Review generated code before using the Apply button.

The default bridge address is `http://127.0.0.1:38247`. You can change it in the plugin panel and start the CLI with `--studio-port <port>`.

## Security model

- the bridge binds to loopback only; it is not exposed to the LAN or internet
- bridge requests require the `X-Roldex-Bridge: studio` header
- request bodies and live script source are size-limited
- file operations remain constrained by the CLI workspace permission mode
- script replacement is an explicit Studio-side action
