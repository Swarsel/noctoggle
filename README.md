# noctoggle

https://github.com/user-attachments/assets/c040c081-0c43-428c-94fb-01d131cabcab

`noctoggle` is a simple utility that shows the [Noctalia Shell](https://github.com/noctalia-dev/noctalia-shell) topbar while a configurable key is held down, and hides it again when the key is released.

## Usage

### NixOS

Add `noctoggle` to your `flake.nix` inputs:

```nix
{
  inputs.noctoggle.url = "github:Swarsel/noctoggle";
}
```

Then, inside a module:

```nix
{ inputs, ... }:
{
  imports = [ inputs.noctoggle.nixosModules.default ];
  services.noctoggle.enable = true;
}
```

### Non-NixOS

1. Build it: `cargo build --release`
2. Setup the service:

```ini
[Unit]
Description=noctoggle – automatic noctalia topbar toggle
After=graphical-session.target
PartOf=graphical-session.target

[Service]
ExecStart=<noctoggle path>
Restart=on-failure
RestartSec=2

[Install]
WantedBy=graphical-session.target
```

### Configuration

To use this out of the box, you will need noctalia v5+, which uses the `noctalia msg bar-show` and `noctalia msg bar-hide` IPC commands together with `reserve_space = false` on the bar. On noctalia-shell v4 (the quickshell-based version), override the show/hide commands with the old syntax (`noctalia-shell ipc call bar showBar` / `hideBar`) and use the `non_exclusive` display mode.

`noctoggle` works by monitoring input devices via `evdev`, which requires read access to `/dev/input/event*`. For that, you will need to add your user to the `input` group.

You can customize the commands executed on key press/release and the trigger keys via environment variables or NixOS options.

- `SHOW_CMD` / `services.noctoggle.showCommand`: Command to run when trigger is pressed (default: `noctalia msg bar-show`)
- `HIDE_CMD` / `services.noctoggle.hideCommand`: Command to run when trigger is released (default: `noctalia msg bar-hide`)
- `TRIGGER_KEYS` / `services.noctoggle.triggerKeys`: Comma-separated list (env) or list of strings (Nix) of keys that trigger the toggle (default: `KEY_LEFTMETA,KEY_RIGHTMETA`). You can also use raw key codes, e.g. `0x30` or `48`.

> [!NOTE]
> If you are using tools like `wev` to find keycodes, be aware that Wayland/X keycodes are shifted by 8 from the kernel keycodes (which are used by `noctoggle`); e.g. `wev` reports key `179` for `XF86Tools`, the actual kernel keycode you should use in `noctoggle` then is `171` (`KEY_CONFIG`).
