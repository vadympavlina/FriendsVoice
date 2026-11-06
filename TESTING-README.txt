FRIENDS VOICE 0.9 - AURORA UI - WINDOWS 10/11 X64
==================================================

VISUAL REDESIGN OF THE NATIVE RUST + EGUI APP.
NO SEPARATE SERVER. Run friends-voice.exe on every connected PC
within the SAME TRUSTED PRIVATE Radmin VPN network (up to 5 friends).

NEW VISUAL EXPERIENCE
- Full premium dark "Aurora" desktop theme with purple, cyan, navy.
- Native drawn brand icon and vector tool buttons.
- Windows Segoe UI font for improved Cyrillic text when available.
- New left sidebar with channel, status, peers, Radmin IP and friend finder.
- Main voice room banner with P2P indicators and responsive gallery.
- Real webcam tiles: centered crop, live indicator, speaking highlight.
- Redesigned chat messages and prominent call control dock.
- Settings panel is resizable and starts closed by default.
- Video gallery adapts to room widths with 1/2/3 columns.
- All existing voice, video, chat, PTT and device settings remain wired.

HOW TO TEST
1. All friends close old versions and extract/run the new EXE.
2. Join one private Radmin VPN network. Verify P2P and friend list.
3. Keep Windows Firewall UDP 45888, UDP 45889, UDP 45890 allowed for
   trusted VPN traffic only. Do not forward ports to public Internet.
4. Join voice, toggle mute/deafen, test F9 and configurable PTT.
5. Open Settings. Select input/output devices, test audio levels,
   individual peer volume, video source and quality presets.
6. Enable a webcam and verify that local/remote video appears inside
   a card and highlights incoming voice. Disable and re-enable.
7. Resize the main window and open/close Settings. Verify gallery,
   chat and call buttons are accessible without overlap.
8. Test with 1, 2, 3, 4 and 5 participants if available.
9. One friend exits; remaining users continue talking without server.

LIMITATIONS
- Windows tests validate program build and algorithms, not visual
  appearance on your physical monitor; please share screenshots.
- Screen sharing is NOT YET implemented: icon intentionally disabled.
- Video capture is JPEG/UDP Beta; no WebRTC/H.264 yet.
- No app-level end-to-end encryption or peer verification.
  Use trusted Radmin VPN only.
- Voice noise gate is optional; echo cancellation is not implemented.
