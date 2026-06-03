# Ous Memory Consolidation System

## Overview

The memory consolidation system in Ous provides three main components for managing and consolidating long-term knowledge from journal entries:

### 1. Daily Notes Module (`daily_notes`)

**Function**: Automatically generates daily summaries of journal entries.

**Features**:
- Runs every hour and checks if it's midnight (UTC 00:00-00:02)
- Reads all entries from yesterday's date in `data/memory/ledger.jsonl`
- Groups entries by type: Captures, Decisions, Projects, Errors
- Generates a Markdown summary to `data/memory/DAILY/YYYY-MM-DD.md`

**Importance Score**: N/A (automatic)

**Example Output**:
```markdown
# Daily Note - 2026-06-02

## Captures
- Fixed bug in AI router
- Updated memory manager interface

## Decisions
- Decided to use Ollama for local AI
- Chose PostgreSQL over SQLite

## Projects
- Memory Consolidation (active)
- Workspace Observer (active)

## Errors
- CPU sensor timeout error
```

**Triggered**: Automatically at midnight UTC

---

### 2. Minor League Module (`minor_league`)

**Function**: Moves low-importance captures to an archive after 30 days.

**Features**:
- Runs daily (every 24 hours)
- Calculates importance score for each entry:
  ```
  score = (recency * 0.2) + (has_approval * 0.5)
  ```
  - **Recency** (20%): Newer entries score higher (max 1.0 at <1 day old, decays to 0.0 at 90 days)
  - **Approval** (50%): User-approved entries score 1.0, others score 0.0
  
- Moves entries with score < 0.4 to `data/memory/MINOR_LEAGUE.jsonl`
- Keeps main ledger clean and focused on high-value entries

**Example Calculation**:
- 45-day-old unapproved entry: score = (0.5 * 0.2) + (0 * 0.5) = 0.1 → moved to minor league
- 15-day-old approved entry: score = (0.83 * 0.2) + (1 * 0.5) = 0.67 → stays in main ledger

**Triggered**: Automatically daily

---

### 3. Dreaming Module (`dreaming`)

**Function**: Uses AI (Ollama) to review journal entries and propose updates to MEMORY.md

**Features**:
- Triggered by a dream packet: `{"type": "dream", "require_approval": true}`
- Reads entries from the last 24 hours
- Reads current `data/memory/MEMORY.md`
- Calls Ollama to summarize and propose memory updates
- Generates JSON proposals with actions: add, update, remove
- Requires user approval before updating MEMORY.md

**Example Dream Packet**:
```json
{
  "type": "dream",
  "require_approval": true
}
```

**Example AI Proposal**:
```json
[
  {
    "action": "add",
    "section": "Architecture Decisions",
    "content": "Use Ollama for all local AI inference to avoid cloud dependencies"
  },
  {
    "action": "add",
    "section": "Project Status",
    "content": "Memory consolidation system now handles daily notes, minor league pruning, and AI-assisted memory updates"
  }
]
```

**Output**: Updated `data/memory/MEMORY.md` with new sections and entries (append-only)

**Triggered**: By sending a dream packet to `data/packets/dream.json`

---

## Directory Structure

```
data/
├── memory/
│   ├── ledger.jsonl              # Main journal (JSONL format)
│   ├── MINOR_LEAGUE.jsonl        # Archived low-importance entries
│   ├── MEMORY.md                 # Curated long-term knowledge (append-only)
│   └── DAILY/
│       ├── 2026-06-02.md         # Daily summary from 2 days ago
│       ├── 2026-06-03.md         # Daily summary from yesterday
│       └── ...
├── packets/                      # Incoming dream packets
│   ├── dream.json               # Dream packet (consumed and archived)
│   └── archive/                 # Archived packets
└── ...
```

---

## Usage Examples

### Manually Trigger Daily Notes
The daily notes run automatically at midnight UTC. To test:
1. Add entries to `data/memory/ledger.jsonl` with yesterday's timestamp
2. Wait for midnight UTC or manually modify the module's timing

### Create a Dream Packet
```json
{
  "type": "dream",
  "require_approval": true
}
```

Save as `data/packets/dream.json` and the system will:
1. Read yesterday's journal entries
2. Read current MEMORY.md
3. Call Ollama to analyze and propose updates
4. Show the proposal and wait for user approval
5. Update MEMORY.md if approved

### Query Minor League Archive
```bash
# Find old, low-importance entries
cat data/memory/MINOR_LEAGUE.jsonl | grep "entity_type"
```

---

## Entry Schema

All journal entries follow this schema:

```json
{
  "entry_id": "uuid",
  "timestamp": "2026-06-03T15:00:00Z",
  "schema_version": 1,
  "entity_type": "capture|decision|project|error",
  "entity_id": "unique-id",
  "operation": "create|update",
  "before_state": null,
  "after_state": {
    "content|summary|name": "value",
    ...
  },
  "reason_capture_id": null,
  "approved_by_user": false
}
```

---

## Integration with Main

All three modules are spawned as async tasks in `main()`:

1. **Daily Notes**: `task::spawn(daily_notes::run_daily_writer(memory_manager.clone()))`
2. **Minor League**: `task::spawn(minor_league::run_pruner(memory_manager.clone()))`
3. **Dreaming**: `task::spawn(dreaming::run_dreaming(bus.clone(), memory_manager.clone(), ai_executor.clone()))`

Each module runs independently and communicates through:
- **MemoryManager**: For reading/writing journal entries
- **MessageBus**: For trigger signals (dreaming)
- **AIExecutor**: For Ollama integration (dreaming)

---

## Verification Checklist

- [x] Daily notes generates Markdown summaries for yesterday's entries
- [x] Minor league calculates importance scores and archives low-scoring entries
- [x] Dreaming calls Ollama and proposes MEMORY.md updates with approval workflow
- [x] All modules compile without errors
- [x] Integration with main() task spawning complete
- [ ] Test with actual Ollama running
- [ ] Verify MEMORY.md created and updated correctly
- [ ] Test edge cases (empty ledger, old entries, etc.)

---

## Token Conservation

The implementation prioritizes token efficiency:
- No multi-line comments or unnecessary docstrings
- Reuses existing `MemoryManager`, `approval`, and `AIExecutor` modules
- Direct JSON parsing without intermediate abstractions
- Streaming writes to avoid loading entire files in memory
