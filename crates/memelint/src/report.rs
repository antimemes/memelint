//! How memelint writes findings on stdout: the default text lines, JSON, or
//! SARIF 2.1.0.

use serde::Serialize;

use crate::check;
use crate::rules;

/// The format of the findings on stdout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// One line for each finding, as in GCC: `file:line:col: severity: message [STE id]`.
    #[default]
    Text,
    /// One JSON object: counts, then the findings.
    Json,
    /// SARIF 2.1.0, which GitHub and other programs read as a report of findings.
    Sarif,
}

/// A finding as a program reads it: path, position, severity, rule, message,
/// and a fix when there is one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Hit {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub severity: String,
    pub rule: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<JsonFix>,
}

/// The replacement that removes a finding, when memelint has one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JsonFix {
    pub start: usize,
    pub end: usize,
    pub replacement: String,
    pub safe: bool,
}

impl From<&check::Fix> for JsonFix {
    fn from(fix: &check::Fix) -> Self {
        Self {
            start: fix.start,
            end: fix.end,
            replacement: fix.replacement.clone(),
            safe: fix.safe,
        }
    }
}

/// The JSON object of `--format json`.
#[derive(Clone, Debug, Serialize)]
pub struct JsonReport {
    /// The ASD-STE100 issue of the vocabulary, when the file has it.
    pub issue: Option<u32>,
    pub files: usize,
    pub comments: usize,
    pub errors: usize,
    pub warnings: usize,
    pub findings: Vec<Hit>,
}

/// The JSON text of the findings, with a line break at the end.
pub fn json(
    hits: &[Hit],
    issue: Option<u32>,
    files: usize,
    comments: usize,
    errors: usize,
    warnings: usize,
) -> String {
    let report = JsonReport {
        issue,
        files,
        comments,
        errors,
        warnings,
        findings: hits.to_vec(),
    };
    let mut text = serde_json::to_string_pretty(&report).expect("Hit serializes");
    text.push('\n');
    text
}

/// The SARIF 2.1.0 text of the findings, with a line break at the end.
pub fn sarif(hits: &[Hit], issue: Option<u32>) -> String {
    let rules: Vec<serde_json::Value> = rules::CHECKED
        .iter()
        .map(|id| {
            let summary = rules::find(id).map(|r| r.summary).unwrap_or("");
            serde_json::json!({
                "id": id,
                "name": format!("STE {id}"),
                "shortDescription": { "text": summary },
                "helpUri": "https://memelint.antimemetics.institute/rules.html",
            })
        })
        .collect();
    let results: Vec<serde_json::Value> = hits
        .iter()
        .map(|h| {
            let level = match h.severity.as_str() {
                "error" => "error",
                _ => "warning",
            };
            serde_json::json!({
                "ruleId": h.rule,
                "level": level,
                "message": { "text": h.message },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": h.path },
                        "region": {
                            "startLine": h.line,
                            "startColumn": h.column,
                        }
                    }
                }],
            })
        })
        .collect();
    let mut driver = serde_json::json!({
        "name": "memelint",
        "version": env!("CARGO_PKG_VERSION"),
        "informationUri": "https://memelint.antimemetics.institute",
        "rules": rules,
    });
    if let Some(issue) = issue {
        driver["properties"] = serde_json::json!({ "asdSte100Issue": issue });
    }
    let doc = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": { "driver": driver },
            "columnKind": "unicodeCodePoints",
            "results": results,
        }],
    });
    let mut text = serde_json::to_string_pretty(&doc).expect("SARIF serializes");
    text.push('\n');
    text
}

/// Write the findings on stdout for `format`. The text format writes each
/// finding on stdout before this function.
pub fn print_machine(
    format: Format,
    hits: &[Hit],
    issue: Option<u32>,
    files: usize,
    comments: usize,
    errors: usize,
    warnings: usize,
) {
    match format {
        Format::Text => {}
        Format::Json => print!("{}", json(hits, issue, files, comments, errors, warnings)),
        Format::Sarif => print!("{}", sarif(hits, issue)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Hit> {
        vec![
            Hit {
                path: "foo.py".into(),
                line: 3,
                column: 7,
                severity: "error".into(),
                rule: "4.2".into(),
                message: "'don't' is a contraction; write the words in full".into(),
                fix: Some(JsonFix {
                    start: 10,
                    end: 15,
                    replacement: "do not".into(),
                    safe: true,
                }),
            },
            Hit {
                path: "foo.py".into(),
                line: 4,
                column: 7,
                severity: "warning".into(),
                rule: "6.3".into(),
                message: "sentence has 28 words; the STE limit for descriptive text is 25".into(),
                fix: None,
            },
        ]
    }

    #[test]
    fn json_has_counts_and_findings() {
        let text = json(&sample(), Some(9), 2, 1, 1, 1);
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["issue"], 9);
        assert_eq!(v["files"], 2);
        assert_eq!(v["comments"], 1);
        assert_eq!(v["errors"], 1);
        assert_eq!(v["warnings"], 1);
        assert_eq!(v["findings"].as_array().unwrap().len(), 2);
        assert_eq!(v["findings"][0]["path"], "foo.py");
        assert_eq!(v["findings"][0]["line"], 3);
        assert_eq!(v["findings"][0]["column"], 7);
        assert_eq!(v["findings"][0]["severity"], "error");
        assert_eq!(v["findings"][0]["rule"], "4.2");
        assert_eq!(v["findings"][0]["fix"]["safe"], true);
        assert_eq!(v["findings"][0]["fix"]["replacement"], "do not");
        assert!(v["findings"][1]["fix"].is_null());
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn sarif_is_version_2_1_0() {
        let text = sarif(&sample(), Some(9));
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["version"], "2.1.0");
        assert_eq!(v["runs"][0]["columnKind"], "unicodeCodePoints");
        let driver = &v["runs"][0]["tool"]["driver"];
        assert_eq!(driver["name"], "memelint");
        assert_eq!(driver["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(driver["properties"]["asdSte100Issue"], 9);
        let ids: Vec<&str> = driver["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, rules::CHECKED);
        let results = v["runs"][0]["results"].as_array().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0]["ruleId"], "4.2");
        assert_eq!(results[0]["level"], "error");
        assert_eq!(results[1]["level"], "warning");
        assert_eq!(
            results[0]["locations"][0]["physicalLocation"]["region"]["startLine"],
            3
        );
        assert_eq!(
            results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
            "foo.py"
        );
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn empty_sarif_has_no_results() {
        let v: serde_json::Value = serde_json::from_str(&sarif(&[], None)).unwrap();
        assert!(v["runs"][0]["results"].as_array().unwrap().is_empty());
        assert!(v["runs"][0]["tool"]["driver"]["properties"].is_null());
    }
}
