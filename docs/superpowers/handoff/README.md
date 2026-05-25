# Session handoff docs

When an autonomous-execution session wraps and a fresh agent needs to pick
up where it left off, the prior session drops a handoff bundle here.

## Files

- [`HANDOFF.md`](./HANDOFF.md) — the start-here doc. Operating contract,
  what's done, what's left, the resume prompt.
- [`pr-ledger.md`](./pr-ledger.md) — frozen snapshot of every PR merged
  in the prior session with SHAs, slugs, and notes.

## When to read

- New autonomous session starts → read both files top-to-bottom before
  picking up any work.
- User explicitly invokes a new agent and asks it to continue → same.
- Mid-session, you forget what shipped → grep this dir.

## When to update

- After your own autonomous-week session wraps, append a new dated
  handoff (don't overwrite the prior one — it's history).
- Conventional naming: `HANDOFF.md` is the current pointer; archive older
  ones as `archive/HANDOFF-YYYY-MM-DD.md` if a new one supersedes it.
