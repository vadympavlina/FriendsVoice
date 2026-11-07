# Friends Voice

**English** · [Українська](README.uk.md) · [Русский](README.ru.md)

A Discord-style voice chat for friends, with no server. A native Windows app written in **Rust + egui**: voice, camera, screen sharing and text chat go straight between the computers of 2–5 friends in a shared **private Radmin VPN network**.

## Getting started
1. Everyone installs Radmin VPN and joins the same trusted network.
2. Download `FriendsVoice-Windows-x64.zip` from [Releases](https://github.com/vadympavlina/FriendsVoice/releases), unpack it and run `friends-voice.exe`.
3. Friends find each other automatically. If not, press “Add a friend by address” in the left panel and enter their `26.x.x.x` address.
4. Click the “General” channel or the “Join voice” button.

UDP ports are needed only inside the VPN: 45888 (discovery and chat), 45889 (voice), 45890 (JPEG camera), 45891 (H.264: HD camera and screen sharing). Never open them to the internet.

## Languages
English (default), Українська and Русский. Switch in Settings → Profile or on the start screen; the change is instant.

## Interface
- **Left:** the voice channel with its people, friends on the network, your Radmin address (with a copy button), and the user bar (microphone, sound, settings). A thin green line over the user bar shows that your microphone hears you.
- **Middle:** the stage with a tile per person and the call controls. The speaker gets a green frame.
- **Right:** chat with search and clickable links. Repeated notices fold into one (“×3”). A quiet sound and a flashing taskbar button tell you about new messages when the window is in the background.

| Key | Action |
|---|---|
| Ctrl+M | microphone |
| F9 | microphone from any window, even in a game |
| Ctrl+B | left panel |
| Ctrl+E | theatre mode (no chat, larger tiles) |
| Ctrl+F | search chat |
| F11 | full screen |
| Esc | step back: full screen, expanded view, search, settings |
| F8 (default) | Push-to-Talk; any key or mouse button can be chosen in Settings → Sound |

## Camera
- Cameras are listed by name. Settings → Video has a **private preview** before you join a call; nothing is sent to friends until you are in the voice channel, and the camera turns off when you leave the page.
- Six quality levels up to **1920×1080 at 30 fps**. The top two (720p and 1080p) use the H.264 codec (about 3 and 6 Mbit/s per friend); the lower ones use JPEG.
- The picture is cropped to 16:9 in the centre, never squashed. Your own preview is mirrored; friends see it normally.

## Screen sharing
The monitor button in the call bar shares **a whole screen or a single window** (Windows Graphics Capture, with the cursor). Resolution 720p / 1080p / 1440p / original up to 4K, 15 / 30 / 60 fps, with a traffic estimate before you start. A friend's screen becomes the big picture with everybody else in a strip; click any tile to enlarge it. Computer sound is not shared yet.

## Expand to the window or the whole screen
Hover a tile with a picture: buttons appear for “fill the window” and “full screen”. Double-click also fills the window; **F11** goes full screen and **Esc** steps back. In the expanded view the picture is always whole, the **mouse wheel zooms up to 600%** and dragging moves it (key 0 resets), which is handy for reading small text. Controls hide when the mouse is still, and a label shows who is talking.

## Sounds
Separate sounds for microphone, sound, camera, screen sharing, your own join/leave, and a distinct bell when a friend joins or leaves. They are synthesized by the app; volume and an on/off switch are in Settings → Sound.

## Limits
There is no end-to-end encryption or identity check; the private VPN is the protection. Echo cancellation is not implemented and computer sound is not shared during screen sharing. Chat history is kept only until the app closes.

## Troubleshooting
Settings → About → “Copy report” gathers the version, devices and network state for a bug report. Crashes are written to `%APPDATA%\FriendsVoice\crash.log` (“Open logs folder” opens it).

## Building
```bash
cargo test
cargo build --release
```
GitHub Actions (`.github/workflows/windows-release.yml`) builds and tests every pull request and publishes a Windows archive with `friends-voice.exe` to Releases after a merge to `main`.
