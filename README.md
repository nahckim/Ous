# Ous

A Rust async personal OS. Captures, classifies, and consolidates knowledge.

## Commands
See COMMANDS.md

## When to use what
- onote: fast capture, no AI
- oguide: Ollama structured note
- oask: DeepSeek query
- odream: trigger dream
- ous: start/restart

## Architecture
- Input: data/packets/, data/guided/
- AI: Ollama local, DeepSeek API
- Memory: data/memory/ledger.jsonl -> MEMORY.md
- Dashboard: 127.0.0.1:8080
