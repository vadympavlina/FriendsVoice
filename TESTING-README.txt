FRIENDS VOICE 0.4.1 - RELIABLE VOICE + NEON DESIGN - WINDOWS 10/11 X64
=================================================

VOICE RELIABILITY FIX:
 - Sends audio ONLY to peers who have joined the voice channel.
 - Keeps UDP receive thread alive on Windows WSAECONNRESET / 10054.
 - Waits for old UDP workers before restarting voice, preventing port race.
 - Settings > Audio shows UDP sent/received packets for connection diagnosis.
 - Test 3 PCs: one joins voice before others, then others join and leave;
   everybody still in voice should hear everybody else without rejoining.
 - If packets sent climb but received stay zero, check inbound UDP 45889
   on EACH computer through the trusted Radmin VPN adapter.

NEW DESIGN:
 - Left: Radmin status, channel, friends, find by IP.
 - Center: real connected participants, chat and bottom voice controls.
 - Right: audio/video/general/about settings tabs.
 - Camera and screen-share buttons are deliberately disabled; not implemented.

THIS RELEASE NEEDS NO SERVER, NO start-server.cmd, NO PASSWORD.
Every computer runs ONLY friends-voice.exe.

1. On every computer: connect to the SAME PRIVATE Radmin VPN network.
2. On every computer: start friends-voice.exe.
3. Allow inbound UDP port 45888 (discovery/chat) and UDP 45889 (voice)
   for the trusted Radmin VPN network in Windows Firewall.
4. Wait until friends appear under "ДРУЗІ В МЕРЕЖІ".
5. If discovery doesn't find someone, add their Radmin 26.x.x.x IPv4
   once with "Знайти за IP". The app saves known friend IPs and shares
   peer addresses to form an independent mesh.
6. Click "Приєднатися до голосу" only when you want to talk.
7. Send messages in the chat without joining voice.
8. Test with THREE PCs: after all are connected, exit the application
   on one PC. The two remaining peers should keep talking normally.

This is a beta:
 - Voice/chat: implemented. Camera and screen sharing: not implemented.
 - UDP discovery relies on virtual-network broadcast support; manual IP
   fallback is provided for Radmin networks that do not relay broadcast.
 - The app does not encrypt or authenticate at the application layer.
   Radmin VPN is the trust boundary. Do NOT use public Radmin networks.
 - The chat is reliable while peers are online, but history is not synced.
 - Up to five computers. Voice uses Opus with default Windows 48 kHz F32 devices.
 - Wear headphones: no echo cancellation yet.
 - If a peer disappears, it can take up to nine seconds to be removed.
