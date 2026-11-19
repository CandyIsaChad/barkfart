//! File kinds and names (port of the Java ProjectStore's naming rules).

/// Sound files the import accepts.
pub const SOUND_EXTENSIONS: [&str; 9] = [".wav", ".mp3", ".m4a", ".aac", ".caf", ".aif", ".aiff", ".ogg", ".flac"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Project,
    Figure,
    Movieclip,
    LegacyProject,
    Pivot,
    Sound,
    Image,
    Zip,
    Other,
}

pub fn kind_of(file_name: &str) -> Kind {
    let n = file_name.to_lowercase();
    if n.ends_with(".bark") {
        return Kind::Project;
    }
    if n.ends_with(".nodes") {
        return Kind::Figure;
    }
    if n.ends_with(".nodemc") {
        return Kind::Movieclip;
    }
    if n.ends_with(".stknds") {
        return Kind::LegacyProject;
    }
    if n.ends_with(".stk") {
        return Kind::Pivot;
    }
    if SOUND_EXTENSIONS.iter().any(|e| n.ends_with(e)) {
        return Kind::Sound;
    }
    if n.ends_with(".png") || n.ends_with(".jpg") || n.ends_with(".jpeg") || n.ends_with(".gif") {
        return Kind::Image;
    }
    if n.ends_with(".zip") {
        return Kind::Zip;
    }
    Kind::Other
}

/// Strips characters not allowed in file names; never empty, at most 80 characters.
pub fn sanitize(name: &str) -> String {
    let s: String = name.chars().filter(|c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && (*c as u32) >= 0x20).collect();
    let s = s.trim().trim_start_matches('.').trim();
    let s: String = s.chars().take(80).collect();
    let s = s.trim().to_string();
    if s.is_empty() {
        "Untitled".into()
    } else {
        s
    }
}
