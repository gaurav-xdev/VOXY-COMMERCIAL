use crate::types::{HarnessError, Result};
use voxy_skills::code_review::{CodeReviewReport, IndependentCodeReviewer};

pub struct Reviewer {
    inner: IndependentCodeReviewer,
}

impl Default for Reviewer {
    fn default() -> Self {
        Self::new()
    }
}

impl Reviewer {
    pub fn new() -> Self {
        Self {
            inner: IndependentCodeReviewer::new(),
        }
    }

    pub fn review_diff(&self, diff_content: &str) -> Result<CodeReviewReport> {
        if diff_content.trim().is_empty() {
            return Err(HarnessError::EditError(
                "Empty diff cannot be reviewed".into(),
            ));
        }

        let report = self.inner.review_diff(diff_content);
        Ok(report)
    }

    pub fn is_blocking(report: &CodeReviewReport) -> bool {
        report.has_blocking_defects
    }
}
