# Third-Party Notices

Roldex is licensed under the MIT License. This document records upstream projects evaluated or referenced while designing optional local-AI support.

## llama.cpp

Project: `ggml-org/llama.cpp`

Purpose in the Roldex architecture: optional local GGUF inference server with an OpenAI-compatible HTTP API.

License: MIT.

Roldex does not currently vendor llama.cpp source code or binaries. Users may install/build llama.cpp separately and point Roldex at its local server.

## OpenAI Codex

Project: `openai/codex`

Purpose in the Roldex architecture: reviewed as an open-source terminal coding-agent reference.

License: Apache License 2.0.

Roldex does not currently vendor Codex source code. Roldex keeps its own Roblox-specialized Rust agent, tool layer, terminal UI, and Studio bridge.

## Qwen Code

Project: `QwenLM/qwen-code`

Purpose in the Roldex architecture: reviewed as an open-source terminal coding-agent reference.

License: Apache License 2.0.

Roldex does not currently vendor Qwen Code source code.

## gpt-oss

Project: `openai/gpt-oss`

Purpose in the Roldex architecture: candidate open-weight local reasoning/agent model family.

License for the upstream repository: Apache License 2.0.

Model weights are not stored in the Roldex repository.

## Model files

Third-party model weights can have licenses or acceptable-use terms that are separate from the inference/runtime code. Roldex does not redistribute model weights. Users should verify the license for the exact model they choose before redistributing it.

If substantial upstream source is vendored into Roldex in the future, its applicable copyright, NOTICE, and license obligations must be preserved alongside that source.
