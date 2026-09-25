//! Previewer for audio/video media attachments.
//!
//! Matches on `audio/*` / `video/*` MIMEs (with a small list of common
//! media extensions as a fallback for files whose MIME is guessed as
//! `application/octet-stream` or missing). The render step emits
//! `PreviewModel::Media { kind }` and the web UI mounts a native
//! `<audio>`/`<video controls>` element bound to the raw-bytes URL —
//! the media plays directly in the page, never converted to text.

use crate::{MediaKind, PreviewContext, PreviewError, PreviewModel, Previewer};

/// Common media extensions matched in addition to `audio/*`/`video/*`
/// MIMEs. `mime_guess` knows most of these but returns `octet-stream`
/// for a handful (e.g. `.mka`, `.opus` on some versions); a suffix
/// check covers the predictable cases.
const MEDIA_EXTS: &[&str] = &[
    // audio
    "mp3", "wav", "ogg", "oga", "opus", "flac", "m4a", "aac", "wma", "mka", "aiff", "mid",
    "midi",
    // video
    "mp4", "m4v", "webm", "mov", "mkv", "avi", "ogv", "mpg", "mpeg", "3gp", "ts", "wmv",
];

const AUDIO_EXTS: &[&str] = &[
    "mp3", "wav", "ogg", "oga", "opus", "flac", "m4a", "aac", "wma", "mka", "aiff", "mid",
    "midi",
];

impl MediaKind {
    fn from_mime(mime: Option<&str>) -> Option<MediaKind> {
        match mime {
            Some(m) if m.starts_with("audio/") => Some(MediaKind::Audio),
            Some(m) if m.starts_with("video/") => Some(MediaKind::Video),
            _ => None,
        }
    }

    fn from_ext(ext: &str) -> MediaKind {
        if AUDIO_EXTS.contains(&ext) {
            MediaKind::Audio
        } else {
            MediaKind::Video
        }
    }
}

/// Previewer for audio/video media attachments.
pub struct MediaPreviewer;

impl Previewer for MediaPreviewer {
    fn id(&self) -> &'static str {
        "media"
    }

    fn matches(&self, ctx: &PreviewContext) -> bool {
        MediaKind::from_mime(ctx.mime.as_deref()).is_some()
            || ctx
                .locator
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase())
                .is_some_and(|e| MEDIA_EXTS.contains(&e.as_str()))
    }

    fn render(&self, ctx: &PreviewContext) -> Result<PreviewModel, PreviewError> {
        let kind = MediaKind::from_mime(ctx.mime.as_deref()).unwrap_or_else(|| {
            // MIME absent/octet-stream: infer from the extension.
            let ext = ctx
                .locator
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            MediaKind::from_ext(&ext)
        });
        Ok(PreviewModel::Media { media: kind })
    }
}
