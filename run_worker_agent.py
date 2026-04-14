"""
Entrypoint for running a single Worker Agent as a standalone process.
Supports TextAgent, ImageAgent, VideoAgent, and WorkflowAgent.

Usage:
    python run_worker_agent.py --type text --id my-text-agent
    python run_worker_agent.py --type image --id my-image-agent
    python run_worker_agent.py --type video --id my-video-agent
    python run_worker_agent.py --type workflow --id my-workflow-agent

Requires:
    Redis server running on localhost:6379
    Start with: redis-server vaultwares-agentciation/redis.conf
"""

import sys
import os
import time
import argparse

sys.path.insert(0, os.path.abspath(os.path.dirname(__file__)))

from agents.text_agent import TextAgent
from agents.image_agent import ImageAgent
from agents.video_agent import VideoAgent
from agents.workflow_agent import WorkflowAgent

AGENT_TYPES = {
    "text": TextAgent,
    "image": ImageAgent,
    "video": VideoAgent,
    "workflow": WorkflowAgent,
}


def main(agent_type: str, agent_id: str):
    agent_class = AGENT_TYPES.get(agent_type)
    if not agent_class:
        print(f"❌ Unknown agent type: '{agent_type}'. Choose from: {list(AGENT_TYPES.keys())}")
        sys.exit(1)

    agent = agent_class(agent_id=agent_id)
    print(f"[Worker Agent: {agent_id} ({agent_type})] Starting...")
    agent.start()
    try:
        while True:
            time.sleep(60)
            print(f"[{agent_id}] Status: {agent.status.value} | Peers: {len(agent._peer_registry)}")
    except KeyboardInterrupt:
        print(f"[Worker Agent: {agent_id}] Shutting down.")
        agent.stop()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Run a VaultWares Worker Agent")
    parser.add_argument(
        "--type",
        type=str,
        required=True,
        choices=list(AGENT_TYPES.keys()),
        help="Agent type to run",
    )
    parser.add_argument(
        "--id",
        type=str,
        required=True,
        help="Unique agent ID",
    )
    args = parser.parse_args()
    main(args.type, args.id)
