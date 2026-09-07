# Roldex architecture

## Product boundary

Roldex is a Roblox Studio development agent, not a general-purpose coding assistant. The terminal application stays lightweight while model inference happens through configurable online providers.

```text
User
  ↓
Roldex CLI
  ├─ session/chat
  ├─ progress UI
  ├─ permissions
  ├─ project context
  └─ tool dispatcher (next milestone)
       ↓
Roldex Core
  ├─ Roblox/Luau specialization
  ├─ provider abstraction
  ├─ filesystem tools
  ├─ project detector
  ├─ Git tools (planned)
  ├─ web/docs tools (planned)
  └─ Studio bridge (planned)
       ↓
Online model provider
```

## Resource strategy

Roldex does not download a large local language model by default. Project scans are bounded, chat history is capped, generated context should be selective, and large files should be streamed or excerpted rather than permanently retained in memory.

## Roblox specialization

The core system prompt encodes Roblox-specific architecture and security rules. Later releases will add retrieval from Roblox Creator Docs, Luau documentation and Rojo documentation, plus deterministic analyzers that do not depend on the model.

## Studio integration

A future `studio-plugin/` package will expose a dockable Roblox Studio widget and a local bridge. The plugin will provide structured context such as current selection, Explorer hierarchy, properties, rigs and animation data. Mutating Studio operations will go through explicit tools rather than raw model access.

## Security model

Default mode is `workspace`:

- absolute filesystem paths are rejected by workspace file tools
- `..` traversal is rejected
- read-only mode disables mutations
- destructive commands will receive additional confirmation/tool policy as the agent executor is implemented

The model never receives unrestricted direct filesystem access; Roldex performs validated tool operations on its behalf.
