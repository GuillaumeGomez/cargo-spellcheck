//! Chunk definition for what is going to be processed by the checkers

use super::*;

use indexmap::IndexMap;

use crate::documentation::PlainOverlay;
use crate::{Range, Span};

/// Definition of the source of a checkable chunk
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ContentOrigin {
    CommonMarkFile(PathBuf),
    RustDocTest(PathBuf, Span), // span is just there to disambiguiate
    RustSourceFile(PathBuf),
}

/// A chunk of documentation that is supposed to be checked
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CheckableChunk {
    /// Rendered contents
    content: String,
    /// Mapping from range within `content` and `Span` referencing the location within the file
    source_mapping: IndexMap<Range, Span>,
}

impl std::hash::Hash for CheckableChunk {
    fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        self.content.hash(hasher);
        // order is consistent
        self.source_mapping.iter().for_each(|t| {
            t.hash(hasher);
        });
    }
}

impl CheckableChunk {
    /// Specific to rust source code, either as part of doc test comments or file scope
    pub fn from_literalset(set: LiteralSet) -> Self {
        set.into_chunk()
    }

    /// Load content from string, may contain markdown content
    pub fn from_str(content: &str, source_mapping: IndexMap<Range, Span>) -> Self {
        Self::from_string(content.to_string(), source_mapping)
    }

    pub fn from_string(content: String, source_mapping: IndexMap<Range, Span>) -> Self {
        Self {
            content,
            source_mapping,
        }
    }

    /// Obtain an accessor object containing mapping and string repr, removing the markdown anotations.
    pub fn erase_markdown(&self) -> PlainOverlay {
        PlainOverlay::erase_markdown(self)
    }

    /// Find which part of the range maps to which span.
    /// Note that Range can very well be split into multiple fragments
    /// where each of them can be mapped to a potentially non-continuous
    /// span
    pub(super) fn find_spans(&self, range: Range) -> IndexMap<Range, Span> {
        let Range { start, end } = range;
        let mut active = false;
        self.source_mapping
            .iter()
            .filter_map(|(range, span)| {
                if range.contains(&start) {
                    active = true;
                    if end > 0 && range.contains(&(end - 1)) {
                        Some(start..end)
                    } else {
                        Some(start..range.end)
                    }
                } else if active {
                    Some(range.clone())
                } else if range.contains(&end) {
                    active = false;
                    Some(range.start..end)
                } else {
                    None
                }
                .map(|fract_range| {
                    // @todo handle multiline here
                    // @todo requires knowledge of how many items are remaining in the line
                    // @todo which needs to be extracted from
                    assert_eq!(span.start.line, span.end.line);
                    let mut span = span.clone();
                    span.start.column += fract_range.start - range.start;
                    span.end.column -= range.end - fract_range.end;
                    assert!(span.start.column <= span.end.column);
                    (fract_range, span)
                })
            })
            .collect::<IndexMap<_, _>>()
    }

    pub fn as_str(&self) -> &str {
        self.content.as_str()
    }
}

/// Convert the clusters of one file into a source description as well
/// as well as vector of checkable chunks.
impl From<Clusters> for Vec<CheckableChunk> {
    fn from(clusters: Clusters) -> Vec<CheckableChunk> {
        clusters
            .set
            .into_iter()
            .map(|literal_set| CheckableChunk::from_literalset(literal_set))
            .collect::<Vec<_>>()
    }
}
