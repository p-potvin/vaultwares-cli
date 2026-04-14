"""
Entrypoint for running the Lonely Manager agent as a standalone process.
This script instantiates the LonelyManager and starts monitoring.

Usage:
    python run_lonely_manager.py

Requires:
    Redis server running on localhost:6379
    Start with: redis-server vaultwares-agentciation/redis.conf
"""

import sys
import os
import time

sys.path.insert(0, os.path.abspath(os.path.dirname(__file__)))

from vaultwares_agentciation import LonelyManager


def alert_callback(alert: dict):
    print(f"\n🚨 [ALERT] {alert['message']}")


if __name__ == "__main__":
    manager = LonelyManager(
        agent_id="lonely_manager",
        alert_callback=alert_callback,
        todo_path="TODO.md",
        roadmap_path="ROADMAP.md",
    )
    print("[Lonely Manager] Starting...")
    manager.start()
    try:
        while True:
            print("\n" + manager.get_project_status_report())
            time.sleep(60)
    except KeyboardInterrupt:
        print("[Lonely Manager] Shutting down.")
        manager._stop_event.set()
