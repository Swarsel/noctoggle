use std::{
    collections::HashSet,
    env,
    path::PathBuf,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicI32, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use evdev::{Device, Key};
use log::{debug, error, info, warn};
use tokio::signal::unix::{SignalKind, signal};
use tokio::{
    io::unix::AsyncFd,
    process::Command,
    sync::mpsc,
    task::JoinSet,
    time::{sleep, timeout},
};

enum CommandAction {
    Show,
    Hide,
}

fn holds_trigger(device: &Device, trigger_keys: &[Key]) -> bool {
    device
        .supported_keys()
        .map(|keys| trigger_keys.iter().any(|&k| keys.contains(k)))
        .unwrap_or(false)
}

fn parse_key(s: &str) -> Option<Key> {
    let s_upper = s.to_uppercase();

    match s_upper.as_str() {
        "SUPER" | "LEFTSUPER" | "SUPER_L" | "META" | "META_L" => return Some(Key::KEY_LEFTMETA),
        "SUPER_R" | "RIGHTSUPER" | "META_R" => return Some(Key::KEY_RIGHTMETA),
        "CTRL" | "CTRL_L" => return Some(Key::KEY_LEFTCTRL),
        "CTRL_R" => return Some(Key::KEY_RIGHTCTRL),
        "ALT" | "ALT_L" => return Some(Key::KEY_LEFTALT),
        "ALT_R" => return Some(Key::KEY_RIGHTALT),
        "SHIFT" | "SHIFT_L" => return Some(Key::KEY_LEFTSHIFT),
        "SHIFT_R" => return Some(Key::KEY_RIGHTSHIFT),
        _ => {
            if let Ok(k) = Key::from_str(&s_upper) {
                return Some(k);
            }
            if let Ok(k) = Key::from_str(&format!("KEY_{}", s_upper)) {
                return Some(k);
            }
            if s.starts_with("0x") {
                u16::from_str_radix(&s[2..], 16).ok().map(Key::new)
            } else {
                s.parse::<u16>().ok().map(Key::new)
            }
        }
    }
}

async fn run_cmd(cmd: &str) {
    debug!("running: {}", cmd);
    let mut parts = cmd.split_whitespace();
    let Some(prog) = parts.next() else { return };
    let args: Vec<&str> = parts.collect();
    match Command::new(prog).args(&args).output().await {
        Ok(o) if o.status.success() => debug!("ok"),
        Ok(o) => error!(
            "failed ({}): {}",
            o.status,
            String::from_utf8_lossy(&o.stderr).trim()
        ),
        Err(e) => error!("spawn error: {}", e),
    }
}

async fn poll_device(
    path: PathBuf,
    device: Device,
    trigger_count: Arc<AtomicI32>,
    trigger_keys: Arc<Vec<Key>>,
    tx: mpsc::Sender<CommandAction>,
) -> Result<()> {
    let name = device.name().unwrap_or("unknown").to_string();
    let mut was_held = false;

    debug!("monitoring {} ({})", path.display(), name);

    let mut events = device.into_event_stream()?;

    while let Ok(ev) = events.next_event().await {
        if ev.event_type() != evdev::EventType::KEY {
            continue;
        }

        let key = Key::new(ev.code());
        if !trigger_keys.contains(&key) && !was_held {
            continue;
        }

        let state = evdev::Device::get_key_state(&events.device_mut())?;
        let held = trigger_keys.iter().any(|&k| state.contains(k));

        if held && !was_held {
            let prev = trigger_count.fetch_add(1, Ordering::AcqRel);
            debug!(
                "Trigger DOWN on {} ({}) (count {} -> {})",
                path.display(),
                name,
                prev,
                prev + 1
            );
            if prev == 0 {
                let _ = tx.send(CommandAction::Show).await;
            }
        } else if !held && was_held {
            let prev = trigger_count.fetch_sub(1, Ordering::AcqRel);
            debug!(
                "Trigger UP on {} ({}) (count {} -> {})",
                path.display(),
                name,
                prev,
                prev - 1
            );
            if prev == 1 {
                let _ = tx.send(CommandAction::Hide).await;
            }
        }

        was_held = held;
    }

    if was_held {
        let prev = trigger_count.fetch_sub(1, Ordering::AcqRel);
        if prev == 1 {
            let _ = tx.send(CommandAction::Hide).await;
        }
    }

    debug!("device {} gone", path.display());
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let show_cmd =
        Arc::new(env::var("SHOW_CMD").unwrap_or_else(|_| "noctalia msg bar-show".to_string()));
    let hide_cmd =
        Arc::new(env::var("HIDE_CMD").unwrap_or_else(|_| "noctalia msg bar-hide".to_string()));

    let trigger_keys_env =
        env::var("TRIGGER_KEYS").unwrap_or_else(|_| "KEY_LEFTMETA,KEY_RIGHTMETA".to_string());
    let trigger_keys: Vec<Key> = trigger_keys_env
        .split(',')
        .filter_map(|s| {
            let trimmed = s.trim();
            let k = parse_key(trimmed);
            if k.is_none() {
                warn!("unknown key '{}'", trimmed);
            }
            k
        })
        .collect();

    if trigger_keys.is_empty() {
        return Err(anyhow::anyhow!("no valid trigger keys configured."));
    }

    let trigger_keys_display = trigger_keys
        .iter()
        .map(|k| format!("{:?}", k))
        .collect::<Vec<_>>()
        .join(", ");
    info!(
        "starting (polling all EV_KEY devices, triggers: {})",
        trigger_keys_display
    );

    let (tx, mut rx) = mpsc::channel::<CommandAction>(32);
    let worker_show = Arc::clone(&show_cmd);
    let worker_hide = Arc::clone(&hide_cmd);

    tokio::spawn(async move {
        while let Some(action) = rx.recv().await {
            match action {
                CommandAction::Show => run_cmd(&worker_show).await,
                CommandAction::Hide => run_cmd(&worker_hide).await,
            }
        }
    });

    let trigger_count: Arc<AtomicI32> = Arc::new(AtomicI32::new(0));
    let trigger_keys = Arc::new(trigger_keys);
    let mut watched: HashSet<PathBuf> = HashSet::new();
    let mut tasks: JoinSet<(PathBuf, Result<()>)> = JoinSet::new();

    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    if fd < 0 {
        return Err(anyhow::anyhow!("failed to initialize inotify"));
    }
    let wd = unsafe {
        libc::inotify_add_watch(
            fd,
            b"/dev/input\0".as_ptr() as *const libc::c_char,
            libc::IN_CREATE,
        )
    };
    if wd < 0 {
        unsafe { libc::close(fd) };
        return Err(anyhow::anyhow!("failed to add inotify watch"));
    }
    let async_fd = AsyncFd::new(fd)?;

    debug!("monitoring /dev/input for new devices");

    loop {
        for (path, device) in evdev::enumerate() {
            if holds_trigger(&device, &trigger_keys) && !watched.contains(&path) {
                let name = device.name().unwrap_or("unknown");
                info!("Found: {} -> {} [holds trigger]", path.display(), name);

                watched.insert(path.clone());
                let count = Arc::clone(&trigger_count);
                let keys = Arc::clone(&trigger_keys);
                let tx_clone = tx.clone();
                let p = path.clone();
                tasks.spawn(async move {
                    let res = poll_device(p.clone(), device, count, keys, tx_clone).await;
                    (p, res)
                });
            }
        }

        tokio::select! {
            _ = sigterm.recv() => {
                info!("SIGTERM received, exiting...");
                break;
            }
            _ = sigint.recv() => {
                info!("SIGINT received, exiting...");
                break;
            }
            res = async_fd.readable() => {
                let mut guard = res?;
                let mut buf = [0u8; 4096];
                let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                if n > 0 {
                    info!("/dev/input changed, re-scanning...");
                }
                guard.clear_ready();
            }
            _ = sleep(Duration::from_secs(60)) => {
            }
            Some(result) = tasks.join_next() => {
                if let Ok((path, _)) = result {
                    watched.remove(&path);
                }
            }
        }
    }

    unsafe {
        libc::inotify_rm_watch(fd, wd);
        libc::close(fd);
    }

    info!("performing cleanup...");
    let _ = timeout(Duration::from_millis(500), run_cmd(&hide_cmd)).await;

    Ok(())
}
