use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

    /// A deterministic id derived from a path. Used for "virtual" folder-notes —
    /// a directory that holds child notes but has no backing `.md` file (and thus
    /// no stored id) yet — so selection stays stable across vault re-walks until
    /// the note is materialized on first edit.
    pub fn from_path(path: &std::path::Path) -> Self {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut h);
        let lo = h.finish();
        "silo-hi".hash(&mut h);
        path.hash(&mut h);
        let hi = h.finish();
        NoteId(Ulid::from(((hi as u128) << 64) | lo as u128))
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

impl Serialize for NoteId {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for NoteId {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frontmatter {
    pub id: NoteId,
    pub created: String, // RFC3339 UTC
    pub updated: String, // RFC3339 UTC
    pub tags: Vec<String>,
    pub pinned: bool,
}

impl Frontmatter {
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    pub fn from_yaml(s: &str) -> Result<Frontmatter, serde_yaml::Error> {
        serde_yaml::from_str(s)
    }
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

    #[test]
    fn now_rfc3339_parses_as_datetime() {
        let s = crate::now_rfc3339();
        assert!(
            chrono::DateTime::parse_from_rfc3339(&s).is_ok(),
            "not rfc3339: {s}"
        );
    }

    #[test]
    fn frontmatter_yaml_roundtrips() {
        let id = NoteId::new();
        let fm = Frontmatter {
            id,
            created: "2026-01-01T00:00:00+00:00".into(),
            updated: "2026-01-02T00:00:00+00:00".into(),
            tags: vec!["a".into(), "b".into()],
            pinned: true,
        };
        let yaml = fm.to_yaml().unwrap();
        let back = Frontmatter::from_yaml(&yaml).unwrap();
        assert_eq!(fm, back);
    }
}
