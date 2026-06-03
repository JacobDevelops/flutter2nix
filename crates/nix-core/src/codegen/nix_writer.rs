/// Generates Nix module expressions from a dependency graph.
pub struct NixExprWriter;

impl NixExprWriter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NixExprWriter {
    fn default() -> Self {
        Self::new()
    }
}
