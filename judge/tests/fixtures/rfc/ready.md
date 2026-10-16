# RFC 0001 — Cache eviction

| Field       | Value            |
| ----------- | ---------------- |
| Status      | In review        |
| Short       | Cache eviction   |
| Risk        | low              |

---

## 1. Summary

Evict cold entries; `load_config()` in `src/thing.py` reads the limit.

```markdown
## 9. Data — an example heading, not a section
`not_a_claim_xyz` is an example
1. an example list item
```

## 4. User-facing design

### 4.1 Configuration

`max_entries` defaults to 1000, the safe side: a smaller cache only costs hits.

## 6. Detailed design

### 6.P Proof against a real client

1. `curl -sI` on an evicted key returned 404, and on a hot key 200.

## 7. Security

Low. Eviction can only delete; `load_config()` caps the limit it reads.

## 8. Supply chain

None: eviction uses only the standard library, so no dependency is added.

## 9. Data and privacy

None: eviction stores no new datum, it only deletes existing entries.

## 10. Operability

### 10.2 Failure modes

If eviction fails the entry stays, which is the safe side.

## 11. Other concerns

Docs: handled in the readme. Accessibility, i18n: no UI, ruled out.

## 14. Test plan

- **Security**: a negative test evicting another tenant's key, refused.

## 15. Decisions and open questions

### Resolved

| # | Question | Decision |
| --- | --- | --- |
| 1 | LRU or LFU | **LRU.** Simpler. |

### Still open

## 16. Implementation phases

One phase.
