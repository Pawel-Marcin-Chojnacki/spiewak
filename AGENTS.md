# Spiewak

Native Windows folder-first music player. The supplied plan is in docs/master-plan.md.

- Work in this single Windows checkout; no worktrees or source copies.
- Push the current clean branch, then create feature/<english-kebab> from HEAD for each feature.
- Push completed features to the existing GitHub origin. Never merge into master or rewrite history.
- Rust owns domain rules, SQLite and audio. C# owns native WinUI presentation.
- Use typed C ABI DTOs; no JSON, IPC, WebView or localhost server.
- Never change music files during indexing. Keep app data in LocalApplicationData/Spiewak.
- Validate Rust tests, fmt and clippy; build and run the WinUI app on Windows.
- Keep docs/STATUS.md honest about implemented and unverified requirements.

- Read docs/household-requirements.md before changing folder/home presentation or metadata automation. Real folders remain canonical; single tagged songs must not become prominent artificial albums.
