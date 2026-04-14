# Agent Manifest

## Multi-Agent Architecture

This repository implements a Redis-based multi-agent coordination system.
All agents inherit from `ExtrovertAgent` and are overseen by `LonelyManager`.

### Core Framework (`vaultwares-agentciation/`)

| Component | File | Description |
|---|---|---|
| **AgentStatus** | `enums.py` | Status enum: LOST, WORKING, WAITING_FOR_INPUT, RELAXING |
| **RedisCoordinator** | `redis_coordinator.py` | Redis pub/sub wrapper (channel: `tasks`, `alerts`) |
| **AgentBase** | `base_agent.py` | Heartbeat loop, status updates, stop/start lifecycle |
| **ExtrovertAgent** | `extrovert_agent.py` | Peer registry, socialization routine, task dispatching |
| **LonelyManager** | `lonely_manager.py` | Heartbeat monitoring, alignment enforcement, task dispatch |

### Specialized AI Agents (`agents/`)

| Agent | File | Type | Skills |
|---|---|---|---|
| **TextAgent** | `agents/text_agent.py` | `text` | Text generation, captioning, prompt engineering, VQA |
| **ImageAgent** | `agents/image_agent.py` | `image` | Image generation, editing, masking, inpainting, outpainting |
| **VideoAgent** | `agents/video_agent.py` | `video` | Video trimming, frame sampling, effects, analysis, captioning |
| **WorkflowAgent** | `agents/workflow_agent.py` | `workflow` | Workflow parsing, ComfyUI/Diffusion export, validation |

### Agent Documentation

- **`agent_text.md`** — Text agent skills, task types, and dispatch examples
- **`agent_image.md`** — Image agent skills, task types, and dispatch examples
- **`agent_video.md`** — Video agent skills, task types, and dispatch examples
- **`agent_workflow.md`** — Workflow agent skills, task types, and dispatch examples
- **`vaultwares-agentciation/skills.md`** — Full skills reference for all Extrovert agents
- **`vaultwares-agentciation/extrovert.agent.md`** — Extrovert personality, Redis rules, status enum
- **`vaultwares-agentciation/lonely-manager.agent.md`** — Manager responsibilities and behaviors

## Redis Coordination

The system uses Redis pub/sub for real-time agent coordination:

- **Channel `tasks`** — all agents subscribe; heartbeats, status, JOIN/LEAVE, ASSIGN messages
- **Channel `alerts`** — LonelyManager publishes LOST agent alerts here
- **Redis hash `lonely_manager:team_state:<agent_id>`** — live team state queryable by external tools

### Message Types

| Action | Publisher | Description |
|---|---|---|
| `HEARTBEAT` | All agents | Sent every 5 seconds with current status |
| `STATUS` | All agents | Sent every 60s or after 3 actions |
| `JOIN` | All agents | Announced on startup |
| `LEAVE` | All agents | Announced on shutdown |
| `ASSIGN` | LonelyManager | Task dispatched to a specific agent |
| `RESULT` | Worker agents | Task result published after completion |
| `ALERT` | LonelyManager | Fired when an agent misses 5+ heartbeats |
| `REALIGN` | LonelyManager | Nudge sent to quiet/drifting agents |
| `REQUEST_UPDATE` | LonelyManager | Periodic status request to all agents |

## Entrypoints

| Script | Description |
|---|---|
| `run_coordinated_system.py` | Start all agents + manager, dispatch demo tasks |
| `run_lonely_manager.py` | Start only the LonelyManager |
| `run_worker_agent.py` | Start a single worker agent by type and ID |

## Quick Start

```bash
# 1. Install dependencies
pip install redis

# 2. Start Redis
redis-server vaultwares-agentciation/redis.conf

# 3. Run the full system
python run_coordinated_system.py

# Or run agents individually:
python run_lonely_manager.py &
python run_worker_agent.py --type text --id text-agent-1 &
python run_worker_agent.py --type image --id image-agent-1 &
```

## Rules & Compliance

All agents follow VaultWares security, privacy, and style guidelines.
See `vaultwares-agentciation/skills.md` for the full rigid rules for Extrovert agents.
