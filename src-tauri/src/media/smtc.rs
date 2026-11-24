use super::{art_data_url, is_target_app, now_ms, track_id, MediaEvent, Track};
use std::sync::mpsc::channel;
use tokio::sync::mpsc::UnboundedSender;
use windows::core::{Interface, HSTRING};
use windows::Foundation::{EventRegistrationToken, TimeSpan, TypedEventHandler};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs, SessionsChangedEventArgs,
    TimelinePropertiesChangedEventArgs,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference, IInputStream};
use windows::Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED};

enum Msg {
    Sessions,
}

/// Event-driven SMTC watcher. One WinRT subscription per interesting event, one
/// re-subscription when Apple Music opens or closes. No timers, no polling.
pub fn run(tx: UnboundedSender<MediaEvent>) {
    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }

    let manager = match request_manager() {
        Some(m) => m,
        None => {
            log::warn!("SMTC manager unavailable; Apple Music detection disabled");
            let _ = tx.send(MediaEvent::Unavailable);
            return;
        }
    };

    let (msg_tx, msg_rx) = channel::<Msg>();
    let sessions_token = manager
        .SessionsChanged(&TypedEventHandler::<Manager, SessionsChangedEventArgs>::new({
            let msg_tx = msg_tx.clone();
            move |_, _| {
                let _ = msg_tx.send(Msg::Sessions);
                Ok(())
            }
        }))
        .ok();

    let mut sub: Option<Sub> = None;
    resync(&manager, &mut sub, &tx);

    while let Ok(Msg::Sessions) = msg_rx.recv() {
        resync(&manager, &mut sub, &tx);
    }

    if let Some(token) = sessions_token {
        let _ = manager.RemoveSessionsChanged(token);
    }
    unsafe { RoUninitialize() };
}

fn request_manager() -> Option<Manager> {
    let m = Manager::RequestAsync().ok()?.get().ok()?;
    Some(m)
}

struct Sub {
    session: Session,
    tokens: [EventRegistrationToken; 3],
}

impl Drop for Sub {
    fn drop(&mut self) {
        let _ = self.session.RemoveMediaPropertiesChanged(self.tokens[0]);
        let _ = self.session.RemovePlaybackInfoChanged(self.tokens[1]);
        let _ = self.session.RemoveTimelinePropertiesChanged(self.tokens[2]);
    }
}

fn resync(manager: &Manager, sub: &mut Option<Sub>, tx: &UnboundedSender<MediaEvent>) {
    let target = find_target(manager);
    let same = match (sub.as_ref(), target.as_ref()) {
        (Some(current), Some(next)) => aumid(&current.session) == aumid(next),
        (None, None) => true,
        _ => false,
    };

    if !same {
        *sub = None; // Drop unsubscribes.
        if let Some(session) = target {
            *sub = subscribe(&session, tx);
        }
    }

    let snapshot = sub.as_ref().and_then(|s| read_session(&s.session));
    let _ = tx.send(MediaEvent::Snapshot(snapshot));
}

fn find_target(manager: &Manager) -> Option<Session> {
    let sessions = manager.GetSessions().ok()?;
    let len = sessions.Size().ok()?;
    for i in 0..len {
        let session = sessions.GetAt(i).ok()?;
        if is_target_app(&aumid(&session)) {
            return Some(session);
        }
    }
    None
}

fn subscribe(session: &Session, tx: &UnboundedSender<MediaEvent>) -> Option<Sub> {
    // Each event needs its own delegate because the argument types differ.
    let props = TypedEventHandler::<Session, MediaPropertiesChangedEventArgs>::new({
        let tx = tx.clone();
        let session = session.clone();
        move |_, _| {
            let _ = tx.send(MediaEvent::Snapshot(read_session(&session)));
            Ok(())
        }
    });
    let playback = TypedEventHandler::<Session, PlaybackInfoChangedEventArgs>::new({
        let tx = tx.clone();
        let session = session.clone();
        move |_, _| {
            let _ = tx.send(MediaEvent::Snapshot(read_session(&session)));
            Ok(())
        }
    });
    let timeline = TypedEventHandler::<Session, TimelinePropertiesChangedEventArgs>::new({
        let tx = tx.clone();
        let session = session.clone();
        move |_, _| {
            let _ = tx.send(MediaEvent::Snapshot(read_session(&session)));
            Ok(())
        }
    });

    let a = session.MediaPropertiesChanged(&props).ok()?;
    let b = session.PlaybackInfoChanged(&playback).ok()?;
    let c = session.TimelinePropertiesChanged(&timeline).ok()?;
    Some(Sub {
        session: session.clone(),
        tokens: [a, b, c],
    })
}

fn aumid(session: &Session) -> String {
    session
        .SourceAppUserModelId()
        .map(|h| h.to_string_lossy())
        .unwrap_or_default()
}

fn read_session(session: &Session) -> Option<Track> {
    let app_id = aumid(session);
    if !is_target_app(&app_id) {
        return None;
    }

    let playback = session.GetPlaybackInfo().ok()?.PlaybackStatus().ok()?;
    let playing = playback == PlaybackStatus::Playing;

    let props = session.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
    let title = text(props.Title());
    if title.trim().is_empty() {
        return None;
    }

    let artist = opt_text(props.Artist());
    let album = opt_text(props.AlbumTitle());
    let (position_ms, duration_ms) = timeline(session);
    let art = props
        .Thumbnail()
        .ok()
        .and_then(read_thumbnail)
        .map(art_data_url);

    Some(Track {
        id: track_id(&app_id, &title, artist.as_deref(), album.as_deref()),
        app_id,
        title,
        artist,
        album,
        art,
        duration_ms,
        position_ms,
        playing,
        position_at: now_ms(),
    })
}

fn timeline(session: &Session) -> (u64, Option<u64>) {
    let Ok(tl) = session.GetTimelineProperties() else {
        return (0, None);
    };
    let start = tl.StartTime().map(span_ms).unwrap_or(0);
    let position = tl.Position().map(span_ms).unwrap_or(0);
    let end = tl.EndTime().map(span_ms).unwrap_or(0);
    let duration = if end > start { Some(end - start) } else { None };
    (position.saturating_sub(start), duration)
}

fn span_ms(span: TimeSpan) -> u64 {
    (span.Duration.max(0) as u64) / 10_000
}

fn text(value: windows::core::Result<HSTRING>) -> String {
    value
        .map(|h| h.to_string_lossy().trim().to_string())
        .unwrap_or_default()
}

fn opt_text(value: windows::core::Result<HSTRING>) -> Option<String> {
    let text = text(value);
    (!text.is_empty()).then_some(text)
}

fn read_thumbnail(reference: IRandomAccessStreamReference) -> Option<Vec<u8>> {
    let stream = reference.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()?;
    // Guard against a surprise multi-MB thumbnail: artwork is UI candy.
    if size == 0 || size > 8_000_000 {
        return None;
    }
    let input: IInputStream = stream.cast().ok()?;
    let reader = DataReader::CreateDataReader(&input).ok()?;
    reader.LoadAsync(size as u32).ok()?.get().ok()?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes).ok()?;
    Some(bytes)
}

// `MediaState` is produced by the engine, not the watcher; keep the import list
// honest without an unused-import warning.
const _: Option<MediaState> = None;
