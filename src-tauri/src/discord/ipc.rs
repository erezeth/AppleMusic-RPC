use super::{client_id_configured, Activity, DiscordState, PresenceCommand, APP_ASSET};
use discord_rich_presence::activity::{Activity as RpcActivity, Assets, Timestamps};
use discord_rich_presence::{DiscordIpc, DiscordIpcClient};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

const CLIENT_ID: &str = super::DISCORD_CLIENT_ID;

/// Owns the Discord named-pipe connection on its own thread: the crate is
/// blocking, and a dedicated thread makes "reconnect" a plain loop.
pub fn spawn(
    rx: UnboundedReceiver<PresenceCommand>,
    retry: Arc<AtomicBool>,
    status: UnboundedSender<DiscordState>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("discord-ipc".into())
        .spawn(move || run(rx, retry, status))
        .expect("failed to spawn Discord thread")
}

fn run(
    mut rx: UnboundedReceiver<PresenceCommand>,
    retry: Arc<AtomicBool>,
    status: UnboundedSender<DiscordState>,
) {
    if !client_id_configured() {
        let _ = status.send(DiscordState::NotConfigured);
        // Still drain the channel so the engine never blocks on a full queue.
        while rx.blocking_recv().is_some() {}
        return;
    }

    let mut desired: Option<Activity> = None;
    let mut sent: Option<Activity> = None;
    let mut client: Option<DiscordIpcClient> = None;
    let mut attempt: u32 = 0;

    loop {
        match rx.try_recv() {
            Ok(cmd) => apply(&mut desired, cmd),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }

        if client.is_none() {
            // Auto-reconnect off: after a failure, only a new command retries.
            if attempt > 0 && !retry.load(Ordering::Relaxed) {
                match rx.blocking_recv() {
                    Some(cmd) => apply(&mut desired, cmd),
                    None => break,
                }
            }
            let _ = status.send(DiscordState::Connecting);
            match connect() {
                Ok(c) => {
                    client = Some(c);
                    attempt = 0;
                    sent = None; // Force one flush on the fresh connection.
                    let _ = status.send(DiscordState::Connected);
                    log::info!("Discord IPC connected");
                }
                Err(err) => {
                    attempt = attempt.saturating_add(1);
                    let _ = status.send(DiscordState::Error(err.clone()));
                    log::warn!("Discord IPC connect failed: {err}");
                    std::thread::sleep(retry_delay(attempt));
                    continue;
                }
            }
        }

        if desired != sent {
            let c = client.as_mut().expect("client checked above");
            let result = match &desired {
                Some(activity) => c.set_activity(to_rpc(activity)),
                None => c.clear_activity(),
            };
            match result {
                Ok(()) => sent = desired.clone(),
                Err(err) => {
                    let _ = status.send(DiscordState::Error(err.to_string()));
                    log::warn!("Discord IPC update failed: {err}");
                    client = None; // Reconnect path above handles it.
                    continue;
                }
            }
        }

        match rx.blocking_recv() {
            Some(cmd) => apply(&mut desired, cmd),
            None => {
                if let Some(mut c) = client.take() {
                    let _ = c.close();
                }
                break;
            }
        }
    }
}

fn apply(desired: &mut Option<Activity>, cmd: PresenceCommand) {
    match cmd {
        PresenceCommand::Set(activity) => *desired = Some(activity),
        PresenceCommand::Clear => *desired = None,
    }
}

fn connect() -> Result<DiscordIpcClient, String> {
    let mut client = DiscordIpcClient::new(CLIENT_ID).map_err(|e| e.to_string())?;
    client.connect().map_err(|e| e.to_string())?;
    Ok(client)
}

fn to_rpc(a: &Activity) -> RpcActivity<'_> {
    let mut rpc = RpcActivity::new()
        .details(a.details.as_str())
        .state(a.state.as_str())
        .assets(
            Assets::new()
                .large_image(APP_ASSET)
                .large_text(a.large_text.as_str()),
        );
    if let (Some(start), Some(end)) = (a.start_ms, a.end_ms) {
        rpc = rpc.timestamps(Timestamps::new().start(start as i64).end(end as i64));
    }
    rpc
}

/// 5s, 10s, 20s, then 30s forever. Discord's IPC is local; fast early retries
/// catch a restart, the cap keeps an idle app quiet.
pub fn retry_delay(attempt: u32) -> Duration {
    Duration::from_secs((5u64 << attempt.min(3)).min(30))
}

#[cfg(test)]
mod tests {
    use super::retry_delay;
    use std::time::Duration;

    #[test]
    fn backoff_grows_then_caps() {
        assert_eq!(retry_delay(0), Duration::from_secs(5));
        assert_eq!(retry_delay(1), Duration::from_secs(10));
        assert_eq!(retry_delay(2), Duration::from_secs(20));
        assert_eq!(retry_delay(3), Duration::from_secs(30));
        assert_eq!(retry_delay(50), Duration::from_secs(30));
    }
}
