# Changelog

All notable changes to Codex project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-11-02

### Added

#### 🌟 3D/4D Git Visualization (Kamui4d超え)
- React Three Fiber + GPU-accelerated rendering
- Real-time monitoring with WebSocket
- Timeline slider with playback controls
- Collaboration features (comments, share links)
- Performance: 50,000 commits @ 35 FPS

#### 🤖 Multi-LLM Support
- OpenAI GPT-5 Pro/Medium/Mini integration
- Anthropic Claude 4.5 Sonnet/Haiku, Claude 4.1 Opus
- Unified AI interface with streaming support
- BYOK (Bring Your Own Key) cost model
- API key encryption (AES-256-GCM)

#### 🔌 Claude Code Integration
- MCP Server implementation
- `@prism` mention support in Claude
- Tools: visualize_repository, analyze_code, get_repo_stats

#### 🔐 Supabase Backend
- Complete authentication system (Email, GitHub OAuth)
- PostgreSQL database with Row Level Security
- Edge Functions for API key management
- Storage buckets for visualizations
- Realtime subscriptions

#### 💰 Zero-Cost Architecture
- Supabase Free Tier: $0/month
- Vercel Free Tier: $0/month
- Cloudflare DNS: $0/month
- Total infrastructure cost: $0.83/month (domain only)

#### 📊 Web Application
- Next.js 14 with App Router
- TypeScript 5.3 with strict type checking
- Tailwind CSS 3.4
- Zustand state management
- Responsive design

### Changed
- **Project renamed**: "Codex" → "Prism" (licensing compliance)
- **Architecture shift**: AWS GPU clusters → Supabase free tier
- **Cost model**: Subscription → BYOK (user-provided API keys)
- **Rust version**: 1.0.0 (from 0.57.0)

### Technical Details

#### Rust CLI
- Workspace version: 1.0.0
- Rust edition: 2024
- Binary name: `codex` (can be renamed to `prism`)
- Cross-platform: Windows, macOS, Linux

#### Web Frontend
- Framework: Next.js 14.0.4
- React: 18.2.0
- Dependencies: 24 packages
- Build target: ES2020

#### MCP Server
- MCP SDK: 0.5.0
- TypeScript: 5.3.3
- Transport: stdio

### Performance
- Rust build time: ~5-10 minutes (release)
- Bundle size (web): <500KB (gzipped)
- 3D visualization: 35 FPS with 50K commits
- Memory usage: 93% reduction vs baseline

### Security
- AES-256-GCM encryption for API keys
- Row Level Security on all tables
- HTTPS enforced (Cloudflare)
- No API keys stored in browser
- Server-side only decryption

### Known Issues
- Release build may cause rustc panic on Windows (use dev build as workaround)
- Some console.log statements remain (will be cleaned in 1.0.1)

### Migration Guide
If upgrading from Codex 0.57.0:
1. Rename binary: `codex` → `prism` (optional)
2. Update config files if using custom paths
3. Re-run `cargo install --path cli --force`

---

## [0.57.0] - 2025-11-01

### Added
- Blueprint Mode complete implementation
- Orchestrator RPC Server with 16 methods
- HMAC-SHA256 authentication
- TypeScript SDK with React hooks
- GUI keyboard shortcuts
- Git worktree support

(See previous versions in git history)

---

## Links

- Repository: https://github.com/zapabob/prism
- Documentation: https://prism.dev/docs
- Issues: https://github.com/zapabob/prism/issues
