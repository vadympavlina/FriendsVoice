FRIENDS VOICE 0.3 - WINDOWS 10/11 X64 - FULL P2P
=================================================

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
