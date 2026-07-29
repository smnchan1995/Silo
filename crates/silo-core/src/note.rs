use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use ulid::Ulid;

/// Stable, device-independent, time-sortable note identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct NoteId(pub Ulid);

impl NoteId {
    pub fn new() -> Self {
        NoteId(Ulid::new())
    }
}

impl Default for NoteId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for NoteId {
    type Err = ulid::DecodeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(NoteId(Ulid::from_string(s)?))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frontmatter {
    pub id: NoteId,
    pub created: String, // RFC3339 UTC
    pub updated: String, // RFC3339 UTC
    pub tags: Vec<String>,
    pub pinned: bool,
}

#[derive(Clone, Debug)]
pub struct Note {
    pub id: NoteId,
    pub path: PathBuf,
    pub title: String,
    pub frontmatter: Frontmatter,
    pub body: String, // markdown, frontmatter stripped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn note_id_roundtrips_through_string() {
        let id = NoteId::new();
        let s = id.to_string();
        assert_eq!(NoteId::from_str(&s).unwrap(), id);
    }

    #[test]
    fn two_new_ids_differ() {
        assert_ne!(NoteId::new(), NoteId::new());
    }
}
