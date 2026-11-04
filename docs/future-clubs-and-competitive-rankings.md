# Deferred: clubs and competitive rankings

Status: **deferred by the user, 2026-11-03**. Do not implement clubs, club memberships, club rankings, ranked eligibility, or a competitive points currency under the current private-table model. The present leaderboard is casual activity/statistics only.

## Reason and reactivation gate

Players choose their opponents and can deliberately trade wins or feed points. Human-only accounts do not prevent collusion or alternate-account farming. Revisit after player population supports automatic random matchmaking and the product has a credible ranked queue and abuse-review policy. Random matchmaking reduces chosen-opponent farming but does not alone guarantee fairness.

The current authorized scope is removing misleading club/member branding and adding daily, weekly, monthly and all-time filters to the existing wins-based casual player leaderboard. Keep existing eligibility and lifetime history. Do not present the filters as competitive certification.

## Preserved investigation

The investigated proposal is a real club system with daily, weekly, monthly and all-time player rankings and club rankings. Club standings sum **all** eligible member contributions; no hidden top-N, averaging, or score normalization.

### Current structural gaps

- player_stats contains cumulative wins, losses, score and games, without dates.
- Existing order is wins, win rate, then total banked score; not points-first competition.
- Human results against bots enter lifetime counters. Guest upgrades can expose their prior casual statistics.
- Tables reuse lobby/game-state identity across rematches. Archives receive UUIDs only when rematching, and their timestamps are archive times, not trustworthy match-completion times.
- Game-state persistence, lobby updates and participant statistics have separate writes with ignored errors. Normal duplicate protection is not a crash-safe accounting guarantee.
- There are no clubs, role permissions or membership-history tables.
- Guest cleanup and foreign-key cascades must not erase future competitive history.
- One mutex-protected SQLite connection and a process-global mutation mutex require bounded leaderboard work; expensive queries can interfere with gameplay.

### Proposed competitive contract, not approved

1. Durable match UUID assigned at start and new UUID for every rematch. Lobby identity remains independent.
2. Ranking points initially equal each eligible participant's final permanent/banked match score, credited once upon completion. Both winner and losers may earn points; temporary points do not count. Keep raw game scores separate from ranking awards.
3. Completed saved-account human-only matches as a possible eligibility baseline. Guest/bot games remain practice. Snapshot eligibility and policy version at start; later guest upgrades never retroactively qualify results.
4. Preserve player count, draw rule and rules/policy versions. Two-, three- and four-player formats have materially different scoring opportunities. Combined raw-point totals describe volume/participation rather than pure skill. Do not invent fairness multipliers without evidence.
5. Calendar day, Monday-start week, calendar month and all time. One published competition timezone; UTC stored timestamps and server-derived half-open boundaries. Asia/Baku was suggested as a product default, not approved. Completion time determines the bucket, including paused matches finishing in a later period. No destructive midnight-reset job.
6. Club attribution captured at match start. A club receives points earned representing it, rather than importing current members' entire lifetime totals. Departures/kicks do not remove earned contributions; joining does not import historical points. Current roster and period contributors are distinct.
7. Example: a player with 10,000 old points joins Club B and earns 200: B receives 200; the player's personal total remains independent; the old club keeps its historical contributions.
8. Shared ranks for equal points, with stable IDs used only for deterministic display. Preserve pagination consistency while results arrive; always expose own rank even outside the top page.

### Clubs as actual product entities

- Permanent club ID, normalized unique name/tag, description, selected emblem/color, creation time, admission mode and lifecycle status.
- One active club per saved player enforced by database constraints, not only UI.
- Roles: owner, officer, member. Exactly one owner; officers manage ordinary admissions and members; ownership transfer precedes owner departure.
- Open join, application, invite-only workflows with explicit acceptance, expiry, capacity checks and audit trail.
- Transactions prevent dual membership, simultaneous last-seat acceptance and conflicting ownership transfers. Recheck role authorization at mutation time.
- Renames preserve identity. Dissolution archives history rather than deleting contributions. Account deletion requires durable non-identifying result identities/tombstones consistent with privacy policy.
- Capacity of 20 was an illustrative recommendation, not approved. A cap alone does not prevent rotating unlimited contributors. Decide whether a scoring roster limits distinct contributors per period; publish the rule visibly.
- Joining cooldown of 24 hours was illustrative, not approved. Avoid blocking membership changes indefinitely behind a paused private match; specify in-flight attribution and abandonment behavior explicitly.

### Fairness and abuse decisions

- Same-club matches can manufacture club points without outside competition. Options: classify them entirely casual, or retain personal activity while excluding club contribution. Must be approved and visible before play.
- Record action participation/timeouts, opponent groups, unusual outcome/volume signals and account histories. Review flags are not automatic proof; shared IP is not grounds by itself.
- Prevent repeated result credit, stale timer actions, concurrent ranked participation exploits and manipulated membership attribution.
- In a ranked queue, define matching policy, queue population requirements, rematch limits, disconnect/forfeit rules, bot substitution eligibility and correction/appeal handling before calling rankings competitive.

### Durable accounting architecture

Proposed entities: matches, snapshotted participants, match_results, participant results/ranking awards, append-only adjustments, clubs, membership intervals, invitations/applications, audit events, transactional notification outbox and rebuildable ranking projections.

Completion must validate match/revision, persist authoritative state, insert the unique final result and all participant awards, update projections and commit together. Only then acknowledge or broadcast. Duplicate identical finalization is harmless; contradictory finalization is an integrity error. Persisted request IDs and revisions protect lost-ACK retries and stale commands; timers carry expected match/revision. Socket delivery remains retryable, not magically exactly once.

Recovery reconciles finished states and ledger records. Aggregates must rebuild exactly from immutable awards. Corrections append linked reversals/adjustments with reason, actor and version; never silently rewrite arbitrary totals. Hiding match history does not affect rankings.

SQLite can remain initially: index completion time, player/club attribution and leaderboard projection order; use bounded reads and short write transactions, benchmark concurrent gameplay. Separate reader connections can benefit from WAL; a single shared connection cannot provide that benefit. Keep blocking database work off async workers. Do not add Redis or migrate databases without measured need.

### Migration

Existing lifetime counters lack dates and mix casual eligibility. Archive dates are not completion dates and archive scores overlap totals already counted. Preserve old statistics as Career record. Introduce an announced verified ranking epoch; only backfill independently verifiable identities/eligibility/times with provenance. Never manufacture daily history or import both archive scores and overlapping aggregate balances. Use numbered, transactional, repeat-safe migrations and fixture-based upgrade tests.

### UX

Primary destinations: Play, Clubs, Rankings. Rankings share Players/Clubs and Today/This week/This month/All time controls, exact ranges, format filters and personal/club positions. Club pages expose Overview, Members, Contributions and authorized management. Every club total drills down to all contributing events, including former contributors. Setup states Ranked or Practice with a reason; results explain player awards and club contribution. Registered account is not synonymous with club member.

### Delivery and acceptance gates

1. Approve competition contract and matchmaking/abuse policies.
2. Durable match accounting and migration before public competitive UI.
3. Complete club lifecycle and authorization.
4. Period player rankings, then club rankings and contribution breakdowns.
5. Home/setup/results integration, localization, accessibility and operational review tools.
6. Test crash/failure at every completion write, retries, restarts, stale commands/timers, rematch uniqueness, reconciliation, period boundaries, ties/pagination, guest upgrade, bot eligibility, account deletion, membership/role races and query latency under gameplay load.

Release invariants: every eligible match credits once; every club total equals its complete contribution ledger; joins/leaves never move past points; legacy migration never invents chronology; and leaderboard traffic does not noticeably delay turns.
