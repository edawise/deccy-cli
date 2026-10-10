//! The refusal value: one failure carrying its evidence, rendered by nothing here.
//!
//! S3-err says failures surface as values, never as exits or panics; S4-own keeps refusal
//! authority with the envelope. So this type is produced below the envelope and rendered by
//! nothing here — it carries its evidence and decides nothing. Rendering lives with the
//! envelope.

/// One refusal: a human-readable cause plus the evidence that produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    cause: String,
    evidence: Vec<String>,
}

impl Refusal {
    /// Build a refusal from its cause.
    pub fn new(cause: impl Into<String>) -> Self {
        Refusal {
            cause: cause.into(),
            evidence: Vec::new(),
        }
    }

    /// Attach one piece of evidence: the offending id, path, or word.
    pub fn with_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.evidence.push(evidence.into());
        self
    }

    /// The cause, read without rendering it.
    pub fn cause(&self) -> &str {
        &self.cause
    }

    /// The evidence, read without rendering it.
    pub fn evidence(&self) -> &[String] {
        &self.evidence
    }
}
