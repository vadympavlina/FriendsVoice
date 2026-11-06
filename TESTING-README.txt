FRIENDS VOICE - WINDOWS 10/11 X64
=================================

Native Rust desktop P2P voice, chat and webcam for up to 5 friends.
No server, no web browser, no installer. Radmin VPN required.
Extract the ZIP and run friends-voice.exe on every participating computer.

LAYOUT
 - Left: voice channel with participants, friends in the network,
   your Radmin VPN address (copy button), user bar (mic, sound, settings).
 - Middle: call stage with participant tiles and call controls.
 - Right: chat with search. Repeated system notices are folded (x3).
 - Settings is a full page: Sound, Video, General, About.

SHORTCUTS
 Ctrl+M mic | Ctrl+B sidebar | Ctrl+E theatre mode | Ctrl+F chat search
 Esc closes search, settings or theatre | F9 global mic | F8 Push-to-Talk

WEBCAM CHECKLIST (please test on a real PC)
 1. Settings > Video: your camera should appear by name. Press
    "Check camera": the preview is mirrored and nobody else can see it.
 2. Leave the Video page without joining a call: the camera must switch off
    (camera light goes out).
 3. Try the three quality levels. Compare delay and smoothness.
 4. Join a call with a friend and turn the camera on from the call bar.
    The friend must see you unmirrored and not stretched.
 5. Switch camera in Settings while it is on: it should restart quickly and
    the window must not freeze.
 6. Test a camera that is used by another app: you should get a clear
    message with the Windows privacy hint, not a silent failure.
 7. Test a laptop camera and, if possible, an external USB camera.
 8. Settings > Video shows "Camera mode: MJPEG 1280x720..." and the frame
    counter must keep growing while the camera is on; "not decoded" must
    stay at 0 (or very low). If it grows, tell us the camera name and mode.
 9. Try the HD quality with a friend and watch smoothness.

PUSH-TO-TALK
 1. Settings > Sound > Change key: press a letter, an F-key, a numpad key
    or a side mouse button. The new key is shown and saved.
 2. Enable Push-to-Talk, join a call, hold the key: the top bar shows who
    is talking. F9 cannot be assigned (reserved for the mic toggle).

GENERAL CHECKLIST
 1. Start at 960x620, 1366x768 and 1920x1080; try 125% and 150% scaling.
 2. Join the voice channel, test mic, sound, Ctrl+M and F9.
 3. Theatre mode (Ctrl+E) with 1, 2, 3 and 5 people; unread chat badge.
 4. Friends can leave and rejoin without ending the call for others.
 5. UDP 45888 (chat/discovery), 45889 (voice), 45890 (webcam) must be
    reachable inside the trusted VPN only.

KNOWN LIMITS
 - Webcam is JPEG over UDP. Screen share is NOT implemented.
 - No echo cancellation, no end-to-end encryption, no identity checks.
 - Chat history is in memory only.
 - Automated CI checks compilation and algorithms, not real webcam/audio
   behaviour or how the UI looks on your monitor.
