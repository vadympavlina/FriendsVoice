FRIENDS VOICE 0.5 - CAMERA BETA + PUSH-TO-TALK
=============================================

ONLY ONE EXE; NO SERVER. WINDOWS 10/11 x64. RADMIN VPN REQUIRED.

1. Extract ZIP. Close older versions of Friends Voice.
2. Have every friend open the new friends-voice.exe on the SAME private
   Radmin VPN network. They should find each other automatically.
3. If missing: Add any friend's 26.x.x.x address with "Знайти за IP".
4. Windows Firewall: allow inbound UDP 45888 (discovery/chat), UDP 45889
   (Opus voice) and UDP 45890 (JPEG webcam) on EACH computer and ONLY
   for trusted Radmin VPN networks. Never expose publicly.
5. Everyone clicks "Увійти в голос"; verify all friends hear each other.
6. "Налаштування -> Аудіо": switch ON "Push-to-Talk (утримувати F8)".
   Hold your selected PTT key (F8 by default) EVEN WHEN GAME IS FOCUSED; release to stop.
   F9 toggles mic globally. Ctrl+M toggles when Friends Voice has focus.
7. In voice click the CAMERA ICON; camera should switch on and local
   video should show on your own tile. Friends see remote video.
   Camera is OFF unless explicitly enabled.
8. Webcam sends low-bandwidth 320x180 JPEG frames up to 6 FPS on UDP.
9. Test with 3 PCs, joining voice and camera in different orders.
   One friend leaves the app; the others must keep their call.

LIMITATIONS
- Video is BETA: not production-grade video call, no adaptive bitrate.
- Screen share is NOT YET available (button remains disabled).
- No app-level end-to-end encryption or identities. TRUSTED VPN ONLY.
- F9 is fixed; PTT can be changed to F6/F7/F8/F10/F11/F12 or Mouse4/Mouse5. Wear headphones for audio.
- Camera uses the first Windows camera. Enable Windows desktop-app
  webcam permissions if capture fails.
