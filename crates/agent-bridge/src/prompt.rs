//! System prompt for live LLM backends.
//!
//! The prompt is stored as a separate Markdown file at
//! `crates/agent-bridge/prompts/system.md` and embedded into the
//! binary via `include_str!`. Keeping it as a Markdown file (rather
//! than an inline `&str`) lets the user proofread + edit it without
//! touching Rust, and lets `cargo doc` link to the rendered source.
//!
//! The prompt is **read-only data**. No runtime; no template
//! substitution at this layer. The per-request nonce sentinel
//! framing for `Observation` data is added by the bridge's request-builder
//! when an actual ACP turn is sent; the prompt only describes the framing
//! contract that the request-builder will use.

/// The system prompt sent to live LLM backends. Embedded at compile
/// time from `prompts/system.md` so it ships with the binary and
/// requires no runtime IO.
pub const SYSTEM_PROMPT: &str = include_str!("../prompts/system.md");

/// Minimum required substrings in the system prompt. Acts as a
/// regression test that the prompt actually covers each required
/// concept. If a future edit removes a critical concept (e.g. the
/// nonce-sentinel framing for XPIA defense), the test fails.
pub const REQUIRED_PROMPT_SUBSTRINGS: &[&str] = &[
    // Tool coverage — every tool from `tools::names::*` must be
    // documented by name in the prompt.
    "no_op",
    "set_speed",
    "place_piece",
    "connect_pieces",
    "remove_piece",
    // Output contract.
    "rationale",
    "confidence",
    // Observation shape must mention the per-request nonce framing used
    // for prompt-injection isolation.
    "OBS-",
    "untrusted observation data",
    // Refusal classifier vocabulary the prompt warns about.
    "refuse",
    "i'm sorry",
    // Engine invariants the model can rely on.
    "deterministic",
    "Warning::InvalidAction",
    // System-prompt-leakage prohibition.
    "NEVER",
];

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_is_non_empty() {
        assert!(
            !SYSTEM_PROMPT.is_empty(),
            "SYSTEM_PROMPT must be embedded; check that prompts/system.md exists"
        );
        assert!(
            SYSTEM_PROMPT.len() > 1000,
            "SYSTEM_PROMPT is suspiciously short ({} bytes); did the include_str! \
             pick up an empty file?",
            SYSTEM_PROMPT.len()
        );
    }

    #[test]
    fn system_prompt_covers_every_required_concept() {
        let mut missing: Vec<&str> = Vec::new();
        for needle in REQUIRED_PROMPT_SUBSTRINGS {
            if !SYSTEM_PROMPT.contains(needle) {
                missing.push(needle);
            }
        }
        assert!(
            missing.is_empty(),
            "SYSTEM_PROMPT is missing required substrings: {missing:?}. \n\
             Either add coverage of the missing concept to prompts/system.md, OR \
             update REQUIRED_PROMPT_SUBSTRINGS if the concept is genuinely no longer required."
        );
    }

    #[test]
    fn system_prompt_documents_every_action_tool() {
        // Cross-check: every tool name from `tools::names` is named in
        // the prompt. If a new tool is added but the prompt isn't
        // updated, this test fails.
        use crate::tools::names;
        for name in [
            names::NO_OP,
            names::SET_SPEED,
            names::PLACE_PIECE,
            names::CONNECT_PIECES,
            names::REMOVE_PIECE,
        ] {
            assert!(
                SYSTEM_PROMPT.contains(name),
                "SYSTEM_PROMPT does not mention the {name:?} tool by name. \
                 Update prompts/system.md to document the tool."
            );
        }
    }

    #[test]
    fn system_prompt_does_not_leak_obvious_secrets() {
        // Defensive: someone might paste a real token while drafting
        // the prompt. This test checks for the most common secret
        // shapes that should never appear in committed code.
        for pattern in [
            "github_pat_",
            "ghp_",
            "ghs_",
            "ghu_",
            "sk-ant-",
            // sk- prefix appears in the doc as a redactor pattern
            // example so we don't ban it here. The above prefixes are
            // more specific and unlikely to be example text.
            "-----BEGIN ",
        ] {
            assert!(
                !SYSTEM_PROMPT.contains(pattern),
                "SYSTEM_PROMPT contains what looks like a real secret prefix {pattern:?}. \
                 Remove the secret from prompts/system.md."
            );
        }
    }
}
