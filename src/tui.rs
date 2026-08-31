//! Focused Volume, Sector, and Page terminal inspector.

use std::fmt;

use crate::inspection::{GraphView, ResourcePolicy};

mod focused;

/// The immutable graph revision displayed when the interactive session exits.
#[must_use]
pub struct TuiExit(focused::FocusedExit);

impl TuiExit {
    /// Return the final explicitly adopted graph revision.
    #[must_use]
    pub fn into_view(self) -> GraphView {
        self.0.into_view()
    }
}

/// A failure to start, drive, or clean up the terminal session.
#[derive(Debug)]
pub struct TuiError(focused::FocusedTerminalError);

impl fmt::Display for TuiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for TuiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Run the focused inspector from one immutable graph revision.
pub fn run(view: GraphView, policy: ResourcePolicy) -> Result<TuiExit, TuiError> {
    focused::run(view, policy).map(TuiExit).map_err(TuiError)
}
