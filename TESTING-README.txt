FRIENDS VOICE 0.6 - QUALITY EDITION - WINDOWS 10/11 X64
=========================================================

Serverless Rust desktop app for up to FIVE friends connected to one
trusted PRIVATE Radmin VPN network. Extract ZIP and run friends-voice.exe.
No separate server process; each peer operates independently.

TEST THIS RELEASE:
1. All friends upgrade to this SAME version. Close older instances.
2. Connect to one private Radmin VPN network. Add a friend's 26.x.x.x IP
   under "Знайти за IP" only if discovery is not working.
3. Windows Firewall per PC: inbound UDP 45888 discovery/chat,
   UDP 45889 voice, UDP 45890 camera. Permit only trusted VPN traffic;
   never forward these ports to the public internet.
4. Join voice. Test F9 mute, Ctrl+M, and optional Push-to-Talk under
   Settings -> Audio (F8 default; F6/F7/F10/F11/F12/Mouse4/Mouse5).
5. In Settings -> Audio change individual friend volume 0–200%.
   Other friends should not be affected by your personal volume.
6. In Settings -> Video select camera 1–4 (index, not device name),
   then click camera to stream. Try three profiles:
       Saver    320x180 @ max 5 fps
       Balanced 320x180 @ max 9 fps
       Sharper  480x270 @ max 8 fps
   Switch camera/quality during a call. Audio should stay connected.
7. Test with 3-5 people, including someone leaving and coming back.
   Verify chat/voice continue and stale camera images disappear.
8. Test with a still scene: video traffic should be reduced while
   camera preview remains visible locally. Changes should stream.

LIMITATIONS:
- Webcam video is beta JPEG/UDP, not H.264/WebRTC, no A/V sync or
  adaptive bandwidth control. Good lighting/low motion helps.
- Camera picker shows numbers 1–4, not Windows device names.
- Screen sharing is NOT implemented; button is disabled.
- No application-level encryption or user authentication. Only use
  trusted private Radmin VPN networks.
- Headphones are recommended (echo cancellation not yet included).
