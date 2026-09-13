# Roldex terminal UI

Roldex uses a compact terminal interface inspired by modern coding agents while keeping its own branding and Roblox-specific workflow.

## Layout

- compact `roldex` header with project/Studio connection state
- scrollable conversation area
- one bordered `Send a message` input at the bottom
- one live progress/status line instead of printing every tool event into the transcript
- elapsed time while a task is active

## Controls

- **Enter** — send the current message
- **Esc while Roldex is working** — cancel the active AI/tool task and immediately return control to the input
- **Esc while idle** — clear the current input
- **Ctrl+C** — exit Roldex
- `/clear` — clear the visible transcript
- `/help` — show available shortcuts
- `/doctor` — inspect local provider/Studio setup

Cancelling a task drops the active provider/tool future. Local processes are configured with kill-on-drop, so processes started by the active task are terminated when cancellation drops their future. An already executing Roblox Studio mutation may finish its current Studio-side operation, but Roldex does not continue the cancelled AI response afterward.

## Response reliability

Provider requests use bounded request/connect timeouts. Temporary network, HTTP 429, and common HTTP 5xx failures receive a small bounded retry budget. If those retries fail, the request returns an error to the UI instead of leaving the terminal indefinitely stuck.

The model can be overridden without editing project files by setting `ROLDEX_MODEL`. Request timeout can be adjusted with `ROLDEX_AI_TIMEOUT_SECONDS` (bounded internally).
