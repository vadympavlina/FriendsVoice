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

SCREEN SHARING (Windows 10 1903+ / Windows 11)
 1. Join a call with a friend. Press the monitor button in the call bar.
 2. Share a whole screen: the friend sees it as the big picture, the other
    people in a strip. Check text sharpness at 1080p/30 and smoothness while
    moving a window or scrolling.
 3. Share one window (e.g. a browser). Minimise it, close it: sharing must
    stop with a clear message. Share a game window if possible.
 4. Try 60 fps and 1440p; watch the banner: it warns when the PC cannot keep up.
 5. Click a small tile to enlarge it, double-click the big one for full screen.
 6. Stop sharing from the banner or the call bar. A friend who joins later must
    see the picture within a second or two.

EXPAND / FULL SCREEN
 1. Hover a camera or screen tile: two buttons appear (fill the window / fill
    the monitor). Double-click a tile also fills the window.
 2. F11 toggles full screen; Esc leaves full screen first, then the expanded view.
 3. In the expanded view: mouse wheel zooms (up to 600%), drag moves, key 0
    resets. Controls fade after ~2.5 s without mouse movement.
 4. If the friend stops sharing, the expanded view closes by itself.

LANGUAGES
 1. First start must be in English. Switch to Українська and Русский on the
    start screen and in Settings > Profile: every screen must change at once
    (rail, stage, chat, all settings pages, tooltips, picker).
 2. If your name was the default (Friend / Друг) it follows the language.
 3. Restart: the chosen language and the window size must be remembered.

CHAT AND NOTIFICATIONS
 1. A link such as https://example.com in a message is clickable and opens the
    browser; punctuation after the link is not part of it.
 2. With the window in the background, a friend's message plays a soft sound
    and flashes the taskbar button.
 3. A thin green line above the user bar moves while you speak.

SOUNDS
 Mic/sound/camera/screen on-off tones, your own join/leave chime, and a
 distinct bell when a friend joins or leaves. Settings > Sound has a test
 panel and a volume slider. F9 (global mic) must play the tone too.

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
