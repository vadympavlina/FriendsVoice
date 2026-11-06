FRIENDS VOICE 0.8 - STABILITY & MEDIA ENGINE - WINDOWS 10/11 X64
===================================================================

IMPORTANT: ALL FRIENDS MUST UPGRADE TO VERSION 0.8.
The voice and video UDP headers have changed and are NOT compatible with
0.7 or older releases. The client warns about incompatible friends.
Do not mix old and new .exe files.

NO SERVER. One friends-voice.exe per PC over the same PRIVATE Radmin VPN.
Up to five connected friends, fully decentralized.

1. Close ALL old instances. Extract and launch this ZIP on EACH PC.
2. Join the same private Radmin VPN network.
3. Permit UDP 45888 (discovery and chat), UDP 45889 (voice),
   UDP 45890 (camera) in Windows Firewall for trusted VPN traffic.
   Do NOT forward these ports to the public Internet.
4. Test ordinary voice with two or more friends. All should hear each
   other on the FIRST voice join. Rejoin several times.
5. Test two people speaking AT ONCE (soft limiting), mute/deafen, F9,
   Ctrl+M and optional configurable Push-to-Talk.
6. Test microphone and headphones on several Windows headsets. The app
   now supports F32/I16/U16 at 48 kHz when available.
7. Settings > Audio > diagnostics displays duplicate/late Opus packets
   and recovered short lost audio frames.
8. Enable 2–4 cameras and verify that video doesn't stall voice.
   Change quality preset and device, turn cameras off/on repeatedly.
9. One friend leaves. The remaining group must keep their call, chat,
   and webcam feeds without a central host.
10. For a long call, monitor CPU/GPU usage and test with unstable VPN.

TESTING NOTES
 - Incoming Opus duplicate/late packets are discarded; up to 2 missing
   frames may be concealed with PLC.
 - Video JPEG decoding runs in a bounded worker queue separate from UDP;
   frame drops under heavy load are preferable to high latency.
 - App only communicates over Radmin VPN; no central server.
 - Noise gate is opt-in and NOT an echo cancellation system.
 - Screen sharing is NOT available yet.
 - No end-to-end encryption or secure peer authentication inside
   Friends Voice. ONLY use trusted private Radmin VPN networks.
