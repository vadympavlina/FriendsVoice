Friends Voice v0.1 - Windows x64
================================

Files:
  friends-voice.exe        - graphical client
  friends-voice-server.exe - TCP server for up to 5 people
  start-server.cmd         - server launcher

TEST ON ONE PC
1. Double-click start-server.cmd, enter a unique password at least 8 characters.
2. Start friends-voice.exe.
3. Server address: 127.0.0.1:45888
4. Enter nickname + password, connect and send a text message.

TEST WITH RADMIN VPN
1. All friends join one private Radmin VPN network.
2. On the host PC, start start-server.cmd and keep it running.
3. Use the host's Radmin VPN IPv4 address, for example 26.x.x.x:45888.
4. Permit inbound TCP 45888 in Windows Firewall for the trusted VPN network.
5. Start friends-voice.exe on each PC and join with nickname + password.

There are no voice calls, video calls, or screen sharing in version 0.1.
IMPORTANT: Chat and passwords are not end-to-end encrypted by this prototype.
Use only in a trusted Radmin VPN network and do not reuse personal passwords.
