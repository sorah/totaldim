# Totaldim

Simple Windows application that sends OSC message to address `/1/mainDim` on `127.0.0.1:7001` with value `1.0f` when Mute key or Ctrl-F10 key is pressed. It means, when local TotalMix instance listens OSC on :7001, pressing such keys toggle [Dim] of Main control room output.

Unfortunately there's no OSC control for Mute, so if we want main output to be muted, we need to set Dim volume to -Inf in TotalMix Settings.

## To-do

- Customizable keybinds
- Accept local/destination udp address in command line arguments

