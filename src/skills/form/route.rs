//! Typed `custom_id` routing.

/// A decoded `custom_id`: which skill owns the interaction, and the path
/// through that skill's workflow.
///
/// Encoded as `skill/seg0/seg1/...`. The first segment is the owning skill (so
/// dispatch can route without prefix-matching); the rest describe the step. The
/// trailing segment may carry an opaque payload (e.g. the name of the item being
/// edited).
///
/// Caveats for later: Discord caps `custom_id` at 100 characters, and segments
/// must not contain `/`. Payloads such as item names should be escaped (or
/// referenced by a short id) before this is production-ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub skill: String,
    pub segments: Vec<String>,
}

impl Route {
    pub fn new(
        skill: impl Into<String>,
        segments: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            skill: skill.into(),
            segments: segments.into_iter().map(Into::into).collect(),
        }
    }

    pub fn encode(&self) -> String {
        let mut out = self.skill.clone();
        for seg in &self.segments {
            out.push('/');
            out.push_str(seg);
        }
        out
    }

    pub fn decode(custom_id: &str) -> Option<Self> {
        let mut parts = custom_id.split('/');
        let skill = parts.next()?.to_string();
        if skill.is_empty() {
            return None;
        }
        Some(Self {
            skill,
            segments: parts.map(str::to_string).collect(),
        })
    }
}
