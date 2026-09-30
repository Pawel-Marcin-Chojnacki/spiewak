# Architecture

One WinUI process loads spiewak_core.dll through a versioned C ABI. Rust owns filesystem, SQLite and playback; the UI never writes music files. Start with purposeful crates, extracting more only when a real responsibility appears.

Windows App SDK 2.5.1 was verified against Microsoft's stable download page and NuGet on 2026-09-30. .NET 10 is installed. ARM64 is the development VM architecture; x64 is also a build target.

Interoptopus 0.16.5 was evaluated. For this small boundary manual sequential DTOs and explicit snapshot ownership avoid a binding-generator build dependency. ABI version checks and Windows integration tests are required before adding commands.
