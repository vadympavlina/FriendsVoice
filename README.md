# Friends Voice 0.2.1 — приватний Discord для друзів

Native Windows 10/11 desktop app in **Rust**, with **Radmin VPN**, max **five friends**. No Electron, no public server, no hosting bill.

## Download

Open [GitHub Releases](https://github.com/vadympavlina/FriendsVoice/releases). Download **FriendsVoice-Windows-x64.zip** from the newest successful test release; extract and run the included executables. You do **not** need Rust, Visual Studio, Cargo or a local compiler.

## Stability improvements in v0.2.1\n\n- Established TCP connections now use blocking reads without a short timeout; clean shutdown happens through the writer socket.\n- The client reports whether the server closed the connection, sent bad data, or caused a socket error.\n- The server logs a specific reason when each user disconnects.\n- A new idle-connection regression test runs before the voice-relay test.\n\n## Main features

- Modern dark UI on `egui`.
- Native voice chat: capture/playback via WASAPI (`cpal`) and compressed Opus audio (48 kHz mono, 20 ms frames).
- Private voice relay over UDP, with random 128-bit credentials minted after the TCP room login. Up to **five** connected friends.
- Mute / deafen and Ctrl+M shortcut, microphone input meter, volume control and selectable audio devices.
- Text chat, connection status, online participants and an access password.
- Remembers host, nickname and device settings in `%APPDATA%/FriendsVoice/settings.json`, **never** the room password.
- Automatic Windows x64 builds, unit / integration tests and releases.

## Get started

1. All friends join the same Radmin VPN private network.
2. On one PC, run `start-server.cmd`, set a unique room password (8+ characters), and keep the server console open.
3. Allow **inbound TCP 45888 and UDP 45889** in Windows Firewall on the host for the trusted VPN network. Do not forward public router ports.
4. Run `friends-voice.exe` on all PCs. Enter host's Radmin VPN IPv4 such as `26.x.x.x:45888`, a unique nickname and room password.
5. Click **Підключитися**, then **Приєднатися до голосу**. Wear headphones to prevent feedback. Test mute/deafen and the chat.
6. To test without Radmin VPN, use `127.0.0.1:45888` on the host computer.

## Requirements and limits

- Windows 10 or 11, audio hardware with **48 kHz F32 support**.
- Everyone must be online in Radmin VPN.
- One host PC acts as server; if it shuts down, the room closes.
- Basic voice jitter buffering; behavior depends on your network and devices.
- No webcam, screensharing, system-audio streaming, AEC or automatic noise cancellation yet.
- The app protocol is **not end-to-end encrypted**. Radmin VPN transports the traffic over its tunnel, but this prototype assumes a trusted private network. Don't reuse important passwords.

## Development

```powershell
cargo test --release --target x86_64-pc-windows-msvc
cargo build --release --target x86_64-pc-windows-msvc
./scripts/smoke-test.ps1
```

GitHub Actions compiles the Windows release and validates TCP handshake, group messages, Opus codec and authenticated UDP relay. Use [Actions](https://github.com/vadympavlina/FriendsVoice/actions) to see CI.

Planned: v0.3 webcam / screen share, then encryption, echo suppression, reconnection, better jitter / packet-loss handling.
