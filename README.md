ALTHOUGH IT IS USABLE IT'S STILL WIP

# Moxnotify

Feature-rich hardware-accelerated keyboard driven Wayland notification daemon.


https://github.com/user-attachments/assets/1996d46c-85cc-4d47-bcf1-5088a58d2192


## Features

- Fully keyboard driven with vi inspired bindings
- Per notification config
- Fully implements [FreeDesktop Notifications Specification][https://specifications.freedesktop.org/notification-spec/latest/]
- Hardware accelerated
- Notification sound support
- Notification history

## Configuration

Moxnotify configuration is written in lua and is located at $XDG_CONFIG_HOME/moxnotify/config.lua or ~/.config/moxnotify/config.lua.

### Example configuration

```lua
return {
  keymaps = {
    {
      mode = "n"
      keys = "d",
      action = "dismiss_notification",
    },
    {
      mode = "n"
      keys = "ge",
      action = "last_notification",
    }
  },
  styles = {
    {
      selector = "*",
      style = {
        border = {
          color = {
            urgency_critical = "#f38ba8",
            urgency_low = "#a6e3a1",
            urgency_normal = "#cba6f7"
          }
        },
        font = {
          color = "#cdd6f4",
          family = "DejaVu Sans",
          size = 10
        }
      }
    },
    {
      selector = {
        "next_counter",
        "prev_counter",
        "notification",
        "hints"
      },
      style = {
        background = {
          urgency_critical = "#181825FF",
          urgency_low = "#1e1e2eFF",
          urgency_normal = "#181825FF"
        }
      }
    },
    {
      selector = "notification",
      state = "hover",
      style = {
        background = {
          urgency_critical = "#313244FF",
          urgency_low = "#313244FF",
          urgency_normal = "#313244FF"
        }
      }
    },
    {
      selector = "action",
      state = "hover",
      style = {
        background = {
          urgency_critical = "#f38ba8",
          urgency_low = "#f2cdcd",
          urgency_normal = "#f2cdcd"
        }
      }
    },
    {
      selector = "progress",
      style = {
        background = {
          urgency_critical = "#f38ba8",
          urgency_low = "#f2cdcd",
          urgency_normal = "#f2cdcd"
        }
      }
    },
    {
      selector = "dismiss",
      style = {
        font = {
          color = "#00000000"
        }
      }
    },
    {
      selector = "dismiss",
      state = "container_hover",
      style = {
        font = {
          color = "#000000"
        }
      }
    }
  }
}
```

## Dependencies

- **Lua** 5.4  
- **Rust**  
- **dbus**
- **wayland**  
- **libpulseaudio**  
- **vulkan-loader**

## Packaging

Moxnotify includes two binaries: the notification daemon (`daemon`) and the control utility (`ctl`). For optimal compatibility with [moxctl](https://github.com/unixpariah/moxctl), rename the binaries as follows:

### Notification Daemon

Rename `daemon` to `moxnotify`:

```bash
cargo build --bin daemon && mv target/release/daemon target/release/moxnotify
```

### Control utility

Rename `ctl` to `moxnotifyctl`:

```bash
cargo build --bin ctl && mv target/release/ctl target/release/moxnotifyctl
```
It is also recommended to package [moxctl](https://github.com/unixpariah/moxctl) together with moxnotify
