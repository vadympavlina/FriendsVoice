# Friends Voice

A private, native **Rust** application for Windows 10/11 and up to **5 friends**, using **Radmin VPN** for connectivity. No Electron.

## Download a Windows release

Open **[Releases](https://github.com/vadympavlina/FriendsVoice/releases)** and download `FriendsVoice-Windows-x64.zip` from the latest successful test build. Extract it, launch `start-server.cmd` on the host PC and `friends-voice.exe` on each participant's computer.

**The app is not compiled on the user's PC.** GitHub Actions builds both Windows executables on hosted Windows runners.

## Radmin VPN setup

1. Join the same private Radmin VPN network on each PC.
2. On one PC, run `start-server.cmd` and set a room password of at least eight characters.
3. Find that host's IPv4 address in Radmin VPN (usually `26.x.x.x`).
4. On other PCs, launch `friends-voice.exe`, enter `26.x.x.x:45888`, a unique nickname, and the room password.
5. If necessary, allow inbound **TCP port 45888** in Windows Firewall for the trusted VPN only; do not forward the port from a public router.
6. To test on one computer, use `127.0.0.1:45888`.

## Current features (v0.1)

- Native dark interface using `eframe`/`egui`.
- Windows client and server.
- Chat, nickname + room password, online members, maximum 5 users.
- Automatic Windows x64 build, server smoke-test and GitHub prerelease on pushes to `main`.

**Not yet implemented:** voice, video, screensharing, app-level encryption. This experimental prototype transfers room password and messages in plaintext inside the private VPN. Only use it with trusted friends. Do not reuse important passwords.

## For developers (not needed to test)

```powershell
cargo build --release --target x86_64-pc-windows-msvc
./scripts/smoke-test.ps1
```

Workflow: `.github/workflows/windows-release.yml`. Every successful push to `main` publishes a new prerelease; tagged commits beginning with `v` produce versioned releases.

Planned milestones: v0.2 voice rooms with Opus; v0.3 camera; v0.4 screen share; v0.5 hardening and encryption.
