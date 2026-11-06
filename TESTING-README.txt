FRIENDS VOICE 0.2.1 - WINDOWS X64
=============================

Files:
 friends-voice.exe        - native Windows GUI (Rust + egui)
 friends-voice-server.exe - private TCP chat / UDP voice server
 start-server.cmd         - easiest server launcher

TEST ON THE SAME COMPUTER
1. Open start-server.cmd and set a unique room password (at least 8 characters).
2. Start friends-voice.exe.
3. Address: 127.0.0.1:45888. Enter your nickname and room password.
4. Connect. Chat and the online user list should work.
5. Press "Приєднатися до голосу" to test local microphone / output device opening.
6. Press mute / deafen or Ctrl+M to toggle the microphone.

TEST WITH TWO WINDOWS PCS AND RADMIN VPN
1. Connect both computers to the same private Radmin VPN network.
2. Only one host needs to run start-server.cmd (leave it open).
3. On the other PC use the host's Radmin VPN IPv4: 26.x.x.x:45888.
4. On host and client launch friends-voice.exe and join with different nicknames.
5. Both friends click "Приєднатися до голосу" to start 2-way audio.
6. Wear headphones to avoid echo.

Windows Firewall may need inbound TCP 45888 and UDP 45889 on the HOST.
Do NOT expose these ports to the public internet.
If your mic doesn't start, open audio settings and select a 48 kHz-capable device.
Audio devices require F32 format at 48 kHz in this release.

FIXED IN 0.2.1\n- More reliable TCP connections while voice chat is active or idle.\n- More specific disconnect message in the client.\n- Server now logs why a client disconnected.\n\nWHAT'S NEW
- Native real-time voice calls with Opus codec, private UDP relay for up to 5 people.
- Mute / deafen, mic meter, volume, Ctrl+M shortcut, input/output choice.
- Redesigned dark interface, remembered address, nickname and device settings.
- Server checks per-session random voice tokens; only logged-in users relay audio.
- Chat, online users and guest password.

LIMITATIONS
- No webcam or screen share yet.
- Encryption is provided by the Radmin VPN tunnel only; the app transport is
  not end-to-end encrypted. Use only inside a trusted VPN and do not reuse passwords.
- No echo cancellation; wear headphones.
- This test version may need manual Windows Firewall allowances.
