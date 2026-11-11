// SPDX-License-Identifier: MPL-2.0

use crate::fl;
use notify_rust::{Hint, Notification};
use tokio::process::Command;

// Pop is the COSMIC sound theme, freedesktop the fallback
const SOUND_FILES: [&str; 2] = [
    "sounds/Pop/stereo/notification/complete.oga",
    "sounds/freedesktop/stereo/complete.oga",
];

pub async fn alert(
    app_id: &'static str,
    summary: String,
    body: String,
    notifications: bool,
    sound: bool,
) {
    if notifications {
        let _ = Notification::new()
            .appname(&fl!("app-title"))
            .summary(&summary)
            .body(&body)
            .icon(app_id)
            .hint(Hint::DesktopEntry(app_id.to_owned()))
            // Keep finished phases out of the notification history
            .hint(Hint::Transient(true))
            // Sound follows the applet's own setting, not the server's
            .hint(Hint::SuppressSound(true))
            .show_async()
            .await;
    }
    if sound {
        play_sound().await;
    }
}

async fn play_sound() {
    let data_dirs = std::env::var_os("XDG_DATA_DIRS")
        .filter(|dirs| !dirs.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let data_dirs: Vec<_> = std::env::split_paths(&data_dirs).collect();
    let Some(path) = SOUND_FILES
        .iter()
        .flat_map(|file| data_dirs.iter().map(move |dir| dir.join(file)))
        .find(|path| path.is_file())
    else {
        eprintln!("no sound file found in the Pop or freedesktop sound theme");
        return;
    };

    // Fall back to PulseAudio when the PipeWire player is missing
    for player in ["pw-play", "paplay"] {
        if Command::new(player).arg(&path).status().await.is_ok() {
            return;
        }
    }
    eprintln!("no sound player found (pw-play or paplay)");
}
