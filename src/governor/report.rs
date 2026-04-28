use std::{collections::BTreeMap, path::PathBuf};

use console::style;

use crate::governor::violation::Violation;

pub fn format_report(violations: &[Violation], files_scanned: usize, verbose: bool) -> String {
    let mut out = String::new();
    if violations.is_empty() {
        out.push_str(&format!("{} governor: clean ({} files scanned)\n", style("✓").green(), files_scanned));
        return out;
    }

    out.push_str(&format!("{} {} governor violations\n\n", style("✗").red(), violations.len()));

    let mut by_file: BTreeMap<PathBuf, Vec<&Violation>> = BTreeMap::new();
    for v in violations {
        by_file.entry(v.file.clone()).or_default().push(v);
    }

    for (file, list) in &by_file {
        for v in list {
            out.push_str(&format!("{}:{}\n    [{}]  {}\n    → {}\n\n", file.display(), v.line_no, style(&v.rule).yellow(), v.snippet, v.suggestion));
        }
    }

    if verbose {
        out.push_str(&format!("\n({} files scanned)\n", files_scanned));
    }
    out
}
