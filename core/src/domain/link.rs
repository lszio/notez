use crate::domain::resource::{ResourceKind, ResourceRef, ResourceRefError};
use serde::{Deserialize, Serialize};

/// Raw link target as found in source text, before resolution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkTarget {
    /// An explicit resource ID: `id:01J...` or `heading:01J...`
    Id {
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        kind_hint: Option<ResourceKind>,
    },
    /// A file-relative or source-relative path: `file:path` or `[label](path)`
    File {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        fragment: Option<String>,
    },
    /// A title/alias match: `[[Some Title]]` or `<<target>>`
    Title {
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        fragment: Option<String>,
    },
    /// An external URL: `https://...`
    Url {
        url: String,
    },
    /// An unknown or custom scheme: `zotero:...`, `denote:...`
    Custom {
        scheme: String,
        value: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        fragment: Option<String>,
    },
}

impl LinkTarget {
    pub fn id(value: impl Into<String>, kind_hint: Option<ResourceKind>) -> Self {
        Self::Id {
            value: value.into(),
            kind_hint,
        }
    }

    pub fn file(path: impl Into<String>, fragment: Option<String>) -> Self {
        Self::File {
            path: path.into(),
            fragment,
        }
    }

    pub fn title(title: impl Into<String>, fragment: Option<String>) -> Self {
        Self::Title {
            title: title.into(),
            fragment,
        }
    }

    pub fn url(url: impl Into<String>) -> Self {
        Self::Url { url: url.into() }
    }

    pub fn custom(
        scheme: impl Into<String>,
        value: impl Into<String>,
        fragment: Option<String>,
    ) -> Self {
        Self::Custom {
            scheme: scheme.into(),
            value: value.into(),
            fragment,
        }
    }

    /// Try to resolve as a direct `ResourceRef` (only possible for `Id` targets
    /// with a known kind_hint).
    pub fn as_resource_ref(&self) -> Option<ResourceRef> {
        match self {
            Self::Id { value, kind_hint } => {
                let kind = (*kind_hint)?;
                let full = format!("{}:{value}", kind.as_str());
                ResourceRef::parse(&full).ok()
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for LinkTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Id { value, kind_hint } => {
                if let Some(k) = kind_hint {
                    write!(f, "{}:{}", k.as_str(), value)
                } else {
                    write!(f, "id:{value}")
                }
            }
            Self::File { path, fragment } => {
                write!(f, "file:{path}")?;
                if let Some(frag) = fragment {
                    write!(f, "::{frag}")?;
                }
                Ok(())
            }
            Self::Title { title, fragment } => {
                write!(f, "[[{title}")?;
                if let Some(frag) = fragment {
                    write!(f, "#{frag}")?;
                }
                write!(f, "]]")
            }
            Self::Url { url } => write!(f, "{url}"),
            Self::Custom {
                scheme,
                value,
                fragment,
            } => {
                write!(f, "{scheme}:{value}")?;
                if let Some(frag) = fragment {
                    write!(f, "::{frag}")?;
                }
                Ok(())
            }
        }
    }
}

/// A text span within a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TextSpan {
    pub line: usize,
    pub col_start: usize,
    pub col_end: usize,
}

/// A link occurrence: one instance of a link in a resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkOccurrence {
    /// The resource containing this link.
    pub source_ref: ResourceRef,
    /// The raw link target.
    pub target: LinkTarget,
    /// The raw text as written in the source document.
    pub raw: String,
    /// Optional display text / label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_text: Option<String>,
    /// Location in the source file.
    pub span: TextSpan,
}

/// Resolution status of a link occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionStatus {
    /// Uniquely resolved to a single resource.
    Resolved,
    /// Multiple candidates found.
    Ambiguous,
    /// No candidate found.
    Unresolved,
    /// External URL, not expected to resolve internally.
    External,
    /// Malformed link target.
    Invalid,
}

/// A resolved relation: one link occurrence that resolved successfully.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedRelation {
    /// The resource containing this link.
    pub source_ref: ResourceRef,
    /// The resolved target resource.
    pub target_ref: ResourceRef,
    /// How the link was expressed.
    pub target: LinkTarget,
    /// Resolution status.
    pub status: ResolutionStatus,
    /// If ambiguous, the candidate refs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<ResourceRef>,
}

/// A `ResourceAddress` is what the public API accepts as input: either a stable
/// ref or a locator that needs resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceAddress {
    Ref {
        #[serde(rename = "ref")]
        r#ref: ResourceRef,
    },
    Locator {
        target: LinkTarget,
    },
}

impl ResourceAddress {
    pub fn from_ref(r: ResourceRef) -> Self {
        Self::Ref { r#ref: r }
    }

    pub fn from_locator(target: LinkTarget) -> Self {
        Self::Locator { target }
    }



    /// Parse a textual address into either a [`ResourceRef`] or a
    /// [`LinkTarget`] locator. Recognized forms:
    /// - `kind:ulid` → `Ref`
    /// - `file:path` or `file:path::fragment` → `Locator::File`
    /// - `id:VALUE` → `Locator::Id`
    /// - `http(s)://...` → `Locator::Url`
    /// - `scheme:value` for any other non-empty scheme → `Locator::Custom`
    /// - `[[Title]]` or `[[Title#frag]]` → `Locator::Title`
    /// - bare text → `Locator::Title`
    pub fn parse(s: &str) -> Result<Self, ResourceRefError> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return Err(ResourceRefError::InvalidFormat);
        }
        // WikiLink form
        if let Some(rest) = trimmed.strip_prefix("[[").and_then(|t| t.strip_suffix("]]")) {
            let (title, fragment) = match rest.split_once('#') {
                Some((t, f)) => (t, Some(f.to_string())),
                None => (rest, None),
            };
            return Ok(Self::Locator {
                target: LinkTarget::title(title, fragment),
            });
        }
        if let Some((kind_str, value)) = trimmed.split_once(':') {
            // Try ResourceRef first
            if matches!(kind_str, "document" | "heading" | "attachment" | "block")
                && let Ok(r_ref) = ResourceRef::parse(trimmed)
            {
                return Ok(Self::Ref { r#ref: r_ref });
            }
            let scheme = kind_str.to_lowercase();
            let (val, fragment) = match value.split_once("::") {
                Some((v, f)) => (v, Some(f.to_string())),
                None => (value, None),
            };
            return Ok(match scheme.as_str() {
                "id" => Self::Locator {
                    target: LinkTarget::id(val, None),
                },
                "file" => Self::Locator {
                    target: LinkTarget::file(val, fragment),
                },
                "http" | "https" => Self::Locator {
                    target: LinkTarget::url(trimmed),
                },
                _ => Self::Locator {
                    target: LinkTarget::custom(kind_str, val, fragment),
                },
            });
        }
        // Bare text → title
        Ok(Self::Locator {
            target: LinkTarget::title(trimmed, None),
        })
    }
}

impl From<ResourceRef> for ResourceAddress {
    fn from(r: ResourceRef) -> Self {
        Self::Ref { r#ref: r }
    }
}

impl From<LinkTarget> for ResourceAddress {
    fn from(target: LinkTarget) -> Self {
        Self::Locator { target }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_target_id_round_trip() {
        let t = LinkTarget::id("01J00000000000000000000001", Some(ResourceKind::Document));
        assert_eq!(t.to_string(), "document:01J00000000000000000000001");
        let r = t.as_resource_ref().unwrap();
        assert_eq!(r.kind(), ResourceKind::Document);
    }

    #[test]
    fn link_target_title_display() {
        let t = LinkTarget::title("My Note", Some("section".into()));
        assert_eq!(t.to_string(), "[[My Note#section]]");
    }

    #[test]
    fn link_target_url() {
        let t = LinkTarget::url("https://example.com");
        assert!(t.as_resource_ref().is_none());
    }

    #[test]
    fn resource_address_from_ref() {
        let r = ResourceRef::parse("document:01J00000000000000000000001").unwrap();
        let addr = ResourceAddress::from(r);
        assert!(matches!(addr, ResourceAddress::Ref { .. }));
    }

    #[test]
    fn resource_address_from_locator() {
        let t = LinkTarget::title("Some Title", None);
        let addr = ResourceAddress::from(t);
        assert!(matches!(addr, ResourceAddress::Locator { .. }));
    }
}
