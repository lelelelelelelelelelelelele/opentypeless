# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased] - 2026-03-31

### Fixed
- **Critical**: Fixed LLM executing user voice commands instead of transcribing
  - Changed from System Prompt to V2-XML User Prompt format
  - All 6 test cases (TC-001 ~ TC-006) now pass, including injection attacks
  - No longer outputs recipes when user mentions "give me a recipe"
  - No longer asks for content when user mentions "clean up scripts"
- Restored selected text compatibility after the V2-XML prompt migration
  - Selected text is again sent as a separate LLM message when context exists
  - Added regression tests for both OpenAI and Cloud LLM providers

### Added
- History view enhancements
  - "View Original" / "Hide Original" buttons to toggle raw transcription display
  - Copy dropdown menu with "Copy Polished" and "Copy Original" options
  - Smooth expand/collapse animations per history entry
  - Added 12 frontend unit tests for History component

### Technical
- Added `build_user_prompt()` function in `src-tauri/src/llm/prompt.rs`
- Implemented XML-style prompt format for better instruction isolation
- Added `anomaly_detector.rs` module (ready but not integrated)

### readme
Gemini 2.5 flash is restricted by google (20 daily use limit). 3.1-flash-lite is the only gemini model with adequent use budget. And the effects have been proved by several border test cases.

### 

## [0.1.0] - 2026-02-26

### Added
- Initial open-source release under MIT license
- Global hotkey voice recording with hold-to-record and toggle modes
- Floating capsule widget — always-on-top, draggable, with recording/transcribing/polishing states
- 6 STT providers: Deepgram Nova-3, AssemblyAI, OpenAI Whisper, Groq Whisper, GLM-ASR, SiliconFlow
- 11 LLM providers: OpenAI, DeepSeek, Zhipu, Claude, Gemini, Moonshot, Qwen, Groq, Ollama, OpenRouter, SiliconFlow
- Real-time streaming keyboard output — text appears character-by-character as the LLM generates it
- Clipboard output mode as alternative to keyboard simulation
- Selected text context — highlight text before recording to give the LLM additional context
- Translation mode — speak in one language, output in another (20+ target languages)
- Custom dictionary for domain-specific terms and proper nouns
- Per-app detection — adapts formatting based on the active application
- Local history with full-text search and date grouping
- Dark / light / system theme with smooth transitions
- Onboarding wizard for first-time setup
- System tray with quick actions (show/hide, start recording, quit)
- Auto-start on login
- Optional Cloud (Pro) subscription for managed STT/LLM quota
- BYOK (Bring Your Own Key) mode — fully functional without any cloud dependency
- Cross-platform support: Windows, macOS, Linux
- CI/CD with automated builds for all three platforms
