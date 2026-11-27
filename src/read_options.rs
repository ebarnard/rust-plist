/// Options for reading plist values, defaulting to a maximum depth of 128.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadOptions {
    pub(crate) max_depth: Option<usize>,
}

impl Default for ReadOptions {
    fn default() -> Self {
        Self {
            max_depth: Some(128),
        }
    }
}

impl ReadOptions {
    /// Creates default read options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum array/dictionary depth; `None` disables the limit.
    /// The root collection counts as one level; `Some(0)` permits only scalars.
    ///
    /// Raising or disabling the limit may cause stack overflow during parsing or cleanup.
    pub fn max_depth(mut self, max_depth: Option<usize>) -> Self {
        self.max_depth = max_depth;
        self
    }
}
