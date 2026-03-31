# OpenTypeless - Agent Documentation

> This document provides essential context for AI coding agents working on OpenTypeless.
> For general project information, see [README.md](./README.md).

## Project Overview

**OpenTypeless** is an open-source, cross-platform desktop application that converts speech to polished text using AI. It combines real-time speech-to-text (STT) with LLM-powered refinement, letting users speak naturally and get well-structured written output in any application.

### Key Features
- Global hotkey recording (hold-to-record or toggle mode)
- Floating capsule widget that stays on top
- Multiple STT providers (Deepgram, AssemblyAI, Whisper variants, Groq, GLM-ASR, SiliconFlow)
- Text polishing via multiple LLMs (OpenAI, DeepSeek, Claude, Gemini, Ollama, etc.)
- Translation mode (20+ languages)
- Custom dictionary for domain-specific terms
- Local history with full-text search
- Keyboard simulation or clipboard output

## Technology Stack

### Frontend
- **Framework**: React 19 with TypeScript 5.8 (strict mode)
- **Build Tool**: Vite 7
- **Styling**: Tailwind CSS 4
- **State Management**: Zustand 5
- **Internationalization**: i18next with react-i18next
- **Animation**: Framer Motion
- **Icons**: Lucide React

### Backend (Rust)
- **Framework**: Tauri 2
- **Audio Capture**: cpal
- **HTTP Client**: reqwest with streaming support
- **Keyboard Simulation**: enigo
- **Database**: SQLite via tauri-plugin-sql and rusqlite
- **Async Runtime**: Tokio

### Key Tauri Plugins
- `tauri-plugin-global-shortcut` - Global hotkey handling
- `tauri-plugin-store` - Local config storage
- `tauri-plugin-sql` - SQLite database
- `tauri-plugin-clipboard-manager` - Clipboard operations
- `tauri-plugin-autostart` - Auto-start on login
- `tauri-plugin-deep-link` - OAuth callback handling
- `tauri-plugin-single-instance` - Prevent multiple instances

## Important Changes

### Prompt Architecture Change (2026-03-31)

**Changed**: Merged upstream's hardened System Prompt architecture, replacing the earlier V2-XML User Prompt experiment.

**Reason**: The original System Prompt caused bugs where the LLM would execute voice commands instead of transcribing them (e.g., "清理脚本碎片" → "请提供需要清理的内容"). We initially fixed this by switching to a single-message V2-XML User Prompt format. Later, upstream adopted equivalent defenses inside the System Prompt architecture — including `<transcription>` tag isolation, explicit SECURITY rules, and input sanitization — making the multi-message V2-XML approach unnecessary to maintain separately.

**Current Architecture**:
- `system` message contains `build_system_prompt()` output with rules, examples, and SECURITY declarations
- `user` message wraps the raw transcript in `<transcription>` tags
- Input-layer hardening: dictionary word sanitization (quotes/newlines stripped) and language-code whitelisting (≤3 alphabetic chars) to prevent injection
- Post-hoc defense: `anomaly_detector.rs` detects unexpected output (length spikes, similarity drops) and triggers a correction retry

**Files Modified**:
- `src-tauri/src/llm/prompt.rs` - `build_system_prompt()` with SECURITY section and input sanitization
- `src-tauri/src/llm/cloud.rs` - Standard system + user multi-message format
- `src-tauri/src/llm/openai.rs` - Standard system + user multi-message format
- `src-tauri/src/llm/anomaly_detector.rs` - Added anomaly detection and correction prompt
- `scripts/test-prompt.mjs` - Integration test script for TC-001 ~ TC-006

**Test Results** (Gemini 3.1 Flash Lite Preview):
- All 7 test cases pass: TC-001 ~ TC-006 (including injection attacks)
- Maintains all polishing functionality (punctuation, filler removal, list formatting)

## Project Structure

```
├── src/                          # React frontend (TypeScript)
│   ├── components/               # UI components
│   │   ├── Capsule/              # Floating widget window
│   │   ├── Settings/             # Settings panels
│   │   ├── History/              # Transcription history
│   │   ├── Onboarding/           # First-run wizard
│   │   └── ...
│   ├── hooks/                    # React hooks
│   ├── stores/                   # Zustand state stores
│   ├── lib/                      # Utilities and API clients
│   ├── i18n/                     # Translation files
│   └── styles/                   # Global CSS
├── src-tauri/src/                # Rust backend
│   ├── audio/                    # Audio capture (cpal)
│   ├── stt/                      # STT provider implementations
│   ├── llm/                      # LLM provider implementations
│   ├── output/                   # Keyboard/clipboard output
│   ├── storage/                  # Config, history, dictionary (SQLite)
│   ├── app_detector/             # Detect active application
│   ├── pipeline.rs               # Core pipeline orchestration
│   └── lib.rs                    # Tauri commands and app setup
└── .github/workflows/            # CI/CD pipelines
```

### Data Flow Pipeline

```
Microphone → Audio Capture → STT Provider → Raw Transcript → LLM Polish → Keyboard/Clipboard Output
```

## Build and Development Commands

### Prerequisites
- Node.js 20+
- Rust stable toolchain
- Platform-specific Tauri prerequisites (see [Tauri docs](https://v2.tauri.app/start/prerequisites/))

### Development
```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Run only frontend dev server
npm run dev
```

### Build
```bash
# Build for production
npm run tauri build

# Output location
# Windows: src-tauri/target/release/bundle/msi/
# macOS:   src-tauri/target/release/bundle/dmg/
# Linux:   src-tauri/target/release/bundle/appimage/ or deb/
```

### Testing
```bash
# Frontend tests (Vitest + jsdom)
npm run test
npm run test:watch

# Rust tests
cargo test --manifest-path src-tauri/Cargo.toml
```

### Linting and Formatting
```bash
# Frontend
npm run lint              # ESLint
npm run format            # Prettier (write)
npm run format:check      # Prettier (check)
npx tsc --noEmit          # TypeScript type check

# Rust
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
```

## Code Style Guidelines

### TypeScript
- **Strict mode enabled**: No `any` types
- **Formatting**: Prettier with single quotes, no semicolons, trailing commas
- **Max line width**: 100 characters
- **Import style**: ES modules, explicit extensions for TypeScript files

### Rust
- Follow standard Rust formatting (`cargo fmt`)
- All warnings treated as errors in CI (`cargo clippy -- -D warnings`)
- Use `anyhow` for error handling
- Use `tracing` for logging

### Conventional Commits
```
<type>: <description>

[optional body]
```

Types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `ci`

Examples:
- `feat: add Groq Whisper STT provider`
- `fix: resolve audio recording crash on macOS`
- `docs: update README installation steps`

## Testing Strategy

### Frontend Tests
- **Framework**: Vitest with jsdom environment
- **Location**: `src/**/*.test.{ts,tsx}`
- **Setup**: `src/test-setup.ts`
- **Coverage**: Component tests for Settings panes, store tests

### Rust Tests
- **Location**: Inline in source files (e.g., `lib.rs`)
- **Examples**: Hotkey parsing tests in `lib.rs`

### CI Pipeline
Runs on PR and push to main:
1. Frontend: type check, lint, format check, unit tests
2. Rust: format check, clippy, tests (Windows, macOS, Ubuntu)
3. Security audit (npm audit, cargo audit)

## Key Configuration Files

| File | Purpose |
|------|---------|
| `package.json` | Node dependencies and scripts |
| `tsconfig.json` | TypeScript compiler options (strict) |
| `vite.config.ts` | Vite build configuration, dev server on port 1420 |
| `vitest.config.ts` | Test configuration |
| `eslint.config.js` | ESLint rules for TypeScript/React |
| `.prettierrc` | Code formatting rules |
| `src-tauri/Cargo.toml` | Rust dependencies and metadata |
| `src-tauri/tauri.conf.json` | Tauri app configuration, window settings |

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_API_BASE_URL` | `https://www.opentypeless.com` | Frontend cloud API base URL |
| `API_BASE_URL` | `https://www.opentypeless.com` | Rust backend cloud API base URL |
| `TAURI_DEV_HOST` | - | Enable HMR on mobile (Tauri dev only) |

## Security Considerations

### BYOK (Bring Your Own Key) Model
- All API keys stored locally via `tauri-plugin-store`
- No mandatory cloud accounts or telemetry
- Audio data sent directly to chosen provider
- CSP enabled in Tauri webview

### macOS Specific
- Requires **Accessibility permission** for keyboard simulation
- App uses `CGEventPost` via `enigo` for typing

### Security Boundaries
- Prompt injection in LLM responses: NOT a vulnerability
- User-exposed API keys through misconfiguration: NOT a vulnerability
- Physical access required: NOT a vulnerability

## Architecture Notes

### Pipeline States
The core recording flow has 5 states:
1. `Idle` - Ready to record
2. `Recording` - Capturing audio
3. `Transcribing` - STT processing
4. `Polishing` - LLM text refinement
5. `Outputting` - Typing/pasting result

### Two-Window Architecture
- **Main window** (`main`): Settings, history, account
- **Capsule window** (`capsule`): Floating widget for recording status
- URL hash `#capsule` determines which view to render

### Storage
- **Config**: `tauri-plugin-store` (JSON files in app data dir)
- **History & Dictionary**: SQLite (`opentypeless.db`)

## Common Tasks

### Adding a New STT Provider
1. Add provider variant in `src-tauri/src/stt/mod.rs`
2. Implement provider in `src-tauri/src/stt/{provider}.rs`
3. Add to provider list in `src/lib/constants.ts`
4. Add connection test in `test_stt_connection` command

### Adding a New LLM Provider
1. Add to `LLM_PROVIDERS` and `LLM_DEFAULT_CONFIG` in `src/lib/constants.ts`
2. Update `test_llm_connection` command if needed
3. Most providers use OpenAI-compatible API

### Adding Translations
1. Add translation file in `src/i18n/locales/{lang}.json`
2. Import and register in `src/i18n/index.ts`
3. Update `LANGUAGES` constant if adding new language option

## Troubleshooting

### Development Issues
- **Hot reload not working**: Check `TAURI_DEV_HOST` for mobile dev
- **Type errors**: Ensure `npm install` ran, check `tsconfig.json` includes
- **Tauri commands not found**: Run `npm install` to install `@tauri-apps/cli`

### Platform-Specific
- **macOS**: Grant Accessibility permission in System Settings for keyboard output
- **Windows**: May need Visual Studio Build Tools for Rust compilation
- **Linux**: Install `libwebkit2gtk-4.1-dev`, `libappindicator3-dev`, `libasound2-dev`

## Resources

- [Tauri v2 Documentation](https://v2.tauri.app/)
- [React Documentation](https://react.dev/)
- [Contributing Guide](./CONTRIBUTING.md)
- [Security Policy](./SECURITY.md)
- [Project Vision](./VISION.md)
