//! # Secret-pattern redactor
//!
//! Pre-write redactor for [`AgentLog`](crate::agent_log) `raw_response`
//! content. Defends against the threat that an LLM echoes back a token
//! inadvertently included in the prompt and AgentLog persists the secret
//! to disk in plaintext.
//!
//! **Single source of truth.** The pattern list below and the
//! drift-detection test in this module are authoritative. Any future PR
//! that adds/removes a pattern must update the test and receive security
//! review.
//!
//! **Markdown vs real regex note.** The secret-redaction policy table column uses
//! `\|` to satisfy GitHub Flavored Markdown table parsing; the
//! authoritative regex strings (BELOW the table) use real `|` for
//! alternation. This module follows the authoritative form. The
//! drift test asserts the actual regex compiled here uses `|` not
//! `\|` (the alternation must produce more than one alternative).
//!
//! **Ordering matters.** More specific patterns run BEFORE more
//! general ones so a `sk-ant-...` Anthropic key is redacted as
//! `anthropic_api_key`, not as `openai_api_key` (which would also
//! match because Anthropic keys start with `sk-`). Once a match has
//! been replaced with `<redacted: NAME>` it cannot be re-matched (the
//! marker contains no characters matched by any pattern).

use std::sync::OnceLock;

use regex::Regex;

/// Stable identifier for a pattern family, embedded into the
/// `<redacted: NAME>` marker so an operator can see WHAT kind of
/// secret was redacted without seeing the value itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatternName(pub &'static str);

impl std::fmt::Display for PatternName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Result of [`redact_secrets`]. `redaction_count` is the total number
/// of matches replaced across all patterns; 0 means the input was
/// unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactionResult {
    pub redacted: String,
    pub redaction_count: usize,
}

/// Build the marker string written in place of a redacted match. The
/// marker is **stable** (no timestamps, no counters) so the same input
/// always redacts to the same output — important for the AgentLog v2
/// determinism / hashing invariants.
#[must_use]
pub fn marker_for(name: PatternName) -> String {
    format!("<redacted: {}>", name.0)
}

/// Authoritative pattern list. Order matters: more specific FIRST.
/// Adding or removing an entry requires updating
/// the drift-detection test in this module in the same PR, with a
/// security-review pass.
const PATTERN_DEFINITIONS: &[(&str, &str)] = &[
    // Anthropic API keys — runs BEFORE openai_api_key because
    // `sk-ant-...` also matches the OpenAI prefix.
    ("anthropic_api_key", r"sk-ant-[A-Za-z0-9-]{32,}"),
    // OpenAI API keys.
    ("openai_api_key", r"sk-[A-Za-z0-9]{20,}"),
    // GitHub fine-grained PAT (more specific than the modern-token
    // family because the prefix is unique).
    ("github_fine_grained_pat", r"github_pat_[A-Za-z0-9_]{82}"),
    // GitHub modern tokens (server, user-to-server, refresh, PAT).
    ("github_modern_token", r"(ghs|ghu|ghr|ghp)_[A-Za-z0-9]{36,}"),
    // Legacy GitHub OAuth.
    ("github_legacy_oauth", r"gho_[A-Za-z0-9]{36}"),
    // AWS access keys.
    ("aws_access_key", r"(AKIA|ASIA)[A-Z0-9]{16}"),
    // Google API keys.
    ("google_api_key", r"AIza[A-Za-z0-9_-]{35}"),
    // Azure OpenAI / Cognitive Services: 32-hex resource-prefixed
    // key adjacent (within ~200 chars) to an Azure domain mention.
    (
        "azure_openai_key",
        r"[a-f0-9]{32}.{0,200}(\.openai\.azure\.com|\.cognitiveservices\.azure\.com)",
    ),
    // JWT three-segment shape.
    (
        "jwt",
        r"eyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+",
    ),
    // PEM private key block (multi-line — `[\s\S]*?` is the
    // non-greedy DOTALL equivalent that doesn't require the `(?s)`
    // inline flag).
    (
        "pem_private_key",
        r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?-----END [A-Z ]*PRIVATE KEY-----",
    ),
];

struct CompiledPattern {
    name: PatternName,
    regex: Regex,
}

static COMPILED: OnceLock<Vec<CompiledPattern>> = OnceLock::new();

fn compiled_patterns() -> &'static [CompiledPattern] {
    COMPILED.get_or_init(|| {
        PATTERN_DEFINITIONS
            .iter()
            .map(|(name, pat)| CompiledPattern {
                name: PatternName(name),
                // Build-time-equivalent: any failure here is a coding
                // error in this module (the patterns are constants).
                // Panic is acceptable on first-call init; will fail
                // every test if a pattern is malformed.
                #[allow(clippy::expect_used)]
                regex: Regex::new(pat).expect("static redactor pattern must compile"),
            })
            .collect()
    })
}

/// Read-only view of the redactor's authoritative pattern list. Use
/// for tests / docs that need to enumerate or display pattern names.
#[must_use]
pub fn pattern_names() -> Vec<PatternName> {
    PATTERN_DEFINITIONS
        .iter()
        .map(|(name, _)| PatternName(name))
        .collect()
}

/// Redact known secret patterns in `input`. Returns the (possibly
/// unchanged) string plus a count of matches replaced.
///
/// **Determinism.** Same input → same output → same redaction_count.
/// Required for the AgentLog write-path hashing invariants (the
/// `state_hash` test runs the entire pipeline twice and asserts
/// equality).
#[must_use]
pub fn redact_secrets(input: &str) -> RedactionResult {
    let mut current = input.to_string();
    let mut count = 0usize;
    for pat in compiled_patterns() {
        let marker = marker_for(pat.name);
        let mut local_count = 0usize;
        // We count via `find_iter` first (cheaper than counting
        // replacements via a closure), then replace.
        for _ in pat.regex.find_iter(&current) {
            local_count += 1;
        }
        if local_count == 0 {
            continue;
        }
        // replace_all returns Cow; force ownership of the result.
        current = pat
            .regex
            .replace_all(&current, marker.as_str())
            .into_owned();
        count += local_count;
    }
    RedactionResult {
        redacted: current,
        redaction_count: count,
    }
}

/// Convenience: just the redacted text.
#[must_use]
pub fn redact_string(input: &str) -> String {
    redact_secrets(input).redacted
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // --- Positive: one example per pattern, all must redact. ---

    #[test]
    fn redacts_github_modern_token_server() {
        let s = "GH=ghs_ZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzAB after";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: github_modern_token>"));
        assert!(!r.redacted.contains("ghs_AbCd"));
    }

    #[test]
    fn redacts_github_fine_grained_pat() {
        // Exactly 82 chars after the prefix.
        let secret = format!("github_pat_{}", "Z".repeat(82));
        let s = format!("token={secret} end");
        let r = redact_secrets(&s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: github_fine_grained_pat>"));
        assert!(!r.redacted.contains(&secret));
    }

    #[test]
    fn redacts_github_legacy_oauth() {
        let s = "gho_ZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZz";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: github_legacy_oauth>"));
    }

    #[test]
    fn redacts_openai_api_key() {
        let s = "OPENAI_API_KEY=sk-ZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZz";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: openai_api_key>"));
    }

    #[test]
    fn redacts_anthropic_key_not_as_openai() {
        // sk-ant-... should match anthropic_api_key FIRST per ordering.
        let s = "key=sk-ant-ZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZz0000end";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(
            r.redacted.contains("<redacted: anthropic_api_key>"),
            "expected anthropic_api_key marker, got: {}",
            r.redacted
        );
        assert!(
            !r.redacted.contains("<redacted: openai_api_key>"),
            "must NOT also fire openai pattern"
        );
    }

    #[test]
    fn redacts_aws_access_key_akia() {
        let s = "AWS=AKIA0Z0Z0Z0Z0Z0Z0Z0Z end";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: aws_access_key>"));
    }

    #[test]
    fn redacts_aws_access_key_asia() {
        let s = "AWS=ASIA0Z0Z0Z0Z0Z0Z0Z0Z end";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: aws_access_key>"));
    }

    #[test]
    fn redacts_google_api_key() {
        // AIza + 35 chars
        let s = "GOOG=AIzaZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZ end";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: google_api_key>"));
    }

    #[test]
    fn redacts_azure_openai_key_adjacent_to_domain() {
        let s = "key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa host=my-resource.openai.azure.com";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: azure_openai_key>"));
    }

    #[test]
    fn redacts_jwt() {
        let s = "tok=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: jwt>"));
    }

    #[test]
    fn redacts_pem_private_key_multiline() {
        let s = "-----BEGIN RSA PRIVATE KEY-----\nlots of base64\nlines here\n-----END RSA PRIVATE KEY-----\nafter";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
        assert!(r.redacted.contains("<redacted: pem_private_key>"));
        assert!(r.redacted.contains("after"));
    }

    // --- Negative: look-alikes that must NOT redact. ---

    #[test]
    fn does_not_redact_sk_underscore_without_dash() {
        let s = "sk_localfunctionname1234567890";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 0, "got: {}", r.redacted);
    }

    #[test]
    fn does_not_redact_bare_40_hex() {
        let s = "git_sha=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa01234567";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 0, "got: {}", r.redacted);
    }

    #[test]
    fn does_not_redact_random_base64_without_jwt_structure() {
        let s = "blob=aGVsbG8gd29ybGQgdGhpcyBpcyBub3QgYSBqd3Q=";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 0);
    }

    #[test]
    fn does_not_redact_short_sk_dash() {
        // sk- with fewer than 20 chars after — under threshold.
        let s = "sk-short";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 0);
    }

    #[test]
    fn does_not_redact_azure_hex_far_from_domain() {
        // 32-hex but >200 chars from azure domain → no match.
        let s = format!(
            "key=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa{} other.openai.azure.com",
            " ".repeat(300)
        );
        let r = redact_secrets(&s);
        assert_eq!(r.redaction_count, 0);
    }

    // --- Multiple patterns at once. ---

    #[test]
    fn redacts_multiple_distinct_patterns_in_one_pass() {
        let s = "gh=ghs_ZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzAB aws=AKIA0Z0Z0Z0Z0Z0Z0Z0Z";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 2);
        assert!(r.redacted.contains("<redacted: github_modern_token>"));
        assert!(r.redacted.contains("<redacted: aws_access_key>"));
    }

    #[test]
    fn redacts_multiple_of_same_pattern() {
        let s = "AKIA0Z0Z0Z0Z0Z0Z0Z0Z and AKIA1Z1Z1Z1Z1Z1Z1Z1Z";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 2);
    }

    // --- Determinism: same input → same output, same count. ---

    #[test]
    fn redaction_is_deterministic() {
        let s = "AKIA0Z0Z0Z0Z0Z0Z0Z0Z and ghs_ZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzAB";
        let r1 = redact_secrets(s);
        let r2 = redact_secrets(s);
        assert_eq!(r1, r2);
    }

    // --- Marker shape (operator-visible signal). ---

    #[test]
    fn marker_includes_pattern_name() {
        for name in pattern_names() {
            let m = marker_for(name);
            assert!(m.starts_with("<redacted: "));
            assert!(m.ends_with('>'));
            assert!(m.contains(name.0));
        }
    }

    #[test]
    fn marker_does_not_match_any_pattern() {
        // Self-inverse: the redaction marker itself must not be matched
        // by any pattern (else we'd double-redact in a future pass).
        for name in pattern_names() {
            let m = marker_for(name);
            let r = redact_secrets(&m);
            assert_eq!(
                r.redaction_count, 0,
                "marker {m:?} accidentally matched a pattern"
            );
        }
    }

    // --- Drift detection against the authoritative pattern list. ---

    #[test]
    fn drift_check_against_authoritative_pattern_list() {
        // EXACT names that define the current redaction surface. If
        // you add/remove a pattern in the redactor, update this list in
        // the same PR and request security-focused review.
        let expected = vec![
            PatternName("anthropic_api_key"),
            PatternName("openai_api_key"),
            PatternName("github_fine_grained_pat"),
            PatternName("github_modern_token"),
            PatternName("github_legacy_oauth"),
            PatternName("aws_access_key"),
            PatternName("google_api_key"),
            PatternName("azure_openai_key"),
            PatternName("jwt"),
            PatternName("pem_private_key"),
        ];
        let actual = pattern_names();
        assert_eq!(
            actual, expected,
            "Redactor pattern list drifted. Update this test with the pattern change."
        );
    }

    #[test]
    fn redactor_uses_true_alternation() {
        // Markdown-escaping foot-gun (secret-redaction policy implementer note):
        // make sure the GitHub modern-token regex uses real `|` not
        // `\|`. If someone copy-pasted from the spec table, the
        // compiled regex would treat the prefix as the literal string
        // `ghs\|ghu\|...` and never match real tokens.
        // Spot-check by feeding `ghr_...` (a non-first alternative).
        let s = "ghr_ZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzZzAB";
        let r = redact_secrets(s);
        assert_eq!(
            r.redaction_count, 1,
            "alternation must accept ALL of (ghs|ghu|ghr|ghp); got {}",
            r.redacted
        );

        // Same for AWS access key — `ASIA` is the second alternative.
        let s = "ASIA0Z0Z0Z0Z0Z0Z0Z0Z";
        let r = redact_secrets(s);
        assert_eq!(r.redaction_count, 1);
    }

    // --- Pipeline invariant: redacted text is short enough that
    //     subsequent AgentLog cap_raw_response doesn't lose marker. ---

    #[test]
    fn marker_is_well_under_raw_response_cap() {
        for name in pattern_names() {
            assert!(
                marker_for(name).len() < 128,
                "marker for {name} unexpectedly long"
            );
        }
    }

    // --- Empty / unchanged inputs. ---

    #[test]
    fn empty_input_is_empty_unchanged() {
        let r = redact_secrets("");
        assert_eq!(r.redacted, "");
        assert_eq!(r.redaction_count, 0);
    }

    #[test]
    fn plain_text_unchanged() {
        let s = "The agent considered three options and chose to do nothing.";
        let r = redact_secrets(s);
        assert_eq!(r.redacted, s);
        assert_eq!(r.redaction_count, 0);
    }
}
