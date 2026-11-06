FRIENDS VOICE 0.7 - CLEAR VOICE + VIDEO PACKET RECOVERY
==========================================================

One native Rust .exe, no separate server. For 2-5 friends in the same
TRUSTED PRIVATE Radmin VPN network (Windows 10/11 x64).

NEW IN v0.7
- Optional noise gate: Settings > Audio > Розумна активація голосом.
  Use the sensitivity slider if voice starts or ends are cut off.
  This setting is OFF by default and does not affect Push-to-Talk.
- JPEG video adds one extra XOR parity packet per multi-part frame to
  reconstruct a frame after ONE lost UDP fragment.
- Video modes and per-friend audio sliders from v0.6 remain available.
- Speaking peer's tile lights up based on actual received voice.
- Video settings show number of parity-recovered JPEG frames.
- GUI uses a lower redraw frequency while idle to save CPU/GPU.
- Added audio noise gate and video parity regression tests.

TEST SCENARIO:
1. Close old Friends Voice programs and extract this release on ALL PCs.
2. Join the same trusted Radmin VPN network.
3. If needed, allow inbound UDP 45888 (presence/chat), UDP 45889
   (Opus audio), UDP 45890 (webcam) for trusted Radmin VPN traffic.
4. With three peers connected, first test ordinary always-on voice.
5. Open Settings > Audio, enable optional noise gate, set sensitivity
   near the left side. Speak loudly, softly and pause between words.
   If syllables disappear, lower threshold or disable the gate.
6. Test Push-to-Talk with F8 (or another chosen key) and F9 mute.
7. Enable at least two cameras; try all three video profiles and
   verify voice remains stable while video is active.
8. Have one friend leave and return while the others remain in voice.

LIMITATIONS:
- Camera is still experimental JPEG/UDP; one parity fragment recovers
  ONE lost data packet, not multiple drops or persistent network loss.
- Screen sharing not implemented yet.
- No app-level end-to-end encryption or identity verification: private,
  trusted Radmin VPN only. Never expose the ports to the Internet.
- Use headphones: automatic echo cancellation not available yet.
