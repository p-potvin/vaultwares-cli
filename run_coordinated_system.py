"""
Entrypoint for the full VaultWares multi-agent coordination system.

Starts:
  - LonelyManager  — monitors heartbeats, enforces alignment, dispatches tasks
  - TextAgent      — text generation, captioning, prompt engineering
  - ImageAgent     — image generation, editing, masking, inpainting
  - VideoAgent     — video trimming, frame sampling, captioning
  - WorkflowAgent  — workflow parsing, conversion, ComfyUI export

Usage:
    python run_coordinated_system.py

Requires:
    Redis server running on localhost:6379
    Start with: redis-server vaultwares-agentciation/redis.conf
"""

import sys
import os
import time

sys.path.insert(0, os.path.abspath(os.path.dirname(__file__)))

from vaultwares_agentciation import LonelyManager
from agents.text_agent import TextAgent
from agents.image_agent import ImageAgent
from agents.video_agent import VideoAgent
from agents.workflow_agent import WorkflowAgent


def alert_callback(alert: dict):
    print(f"\n🚨 [SYSTEM ALERT] {alert['message']}")


def main():
    print("=" * 60)
    print("  VaultWares Multi-Agent Coordination System")
    print("=" * 60)
    print()

    # ----------------------------------------------------------------
    # Start the Lonely Manager (monitors heartbeats, dispatches tasks)
    # ----------------------------------------------------------------
    manager = LonelyManager(
        agent_id="lonely_manager",
        alert_callback=alert_callback,
        todo_path="TODO.md",
        roadmap_path="ROADMAP.md",
    )
    manager.start()
    print(f"✅ Manager '{manager.agent_id}' started.")

    # ----------------------------------------------------------------
    # Start the specialized worker agents
    # ----------------------------------------------------------------
    text_agent = TextAgent(agent_id="text-agent")
    image_agent = ImageAgent(agent_id="image-agent")
    video_agent = VideoAgent(agent_id="video-agent")
    workflow_agent = WorkflowAgent(agent_id="workflow-agent")

    for agent in (text_agent, image_agent, video_agent, workflow_agent):
        agent.start()
        print(f"✅ Agent '{agent.agent_id}' started.")

    print()
    print("--- All agents are running. Redis heartbeat active. ---")
    print("--- Press Ctrl+C to stop all agents. ---")
    print()

    # ----------------------------------------------------------------
    # Optional: dispatch a demo task set after 3 seconds
    # ----------------------------------------------------------------
    time.sleep(3)
    print("\n📤 [Demo] Dispatching sample tasks to agents...\n")

    manager.assign_task(
        "text-agent",
        "generate_caption",
        description="Generate a detailed caption for a sample image",
        source="sample.jpg",
        caption_style="detailed",
    )
    manager.assign_task(
        "image-agent",
        "generate_image",
        description="Generate a test image using SDXL",
        prompt="A futuristic city at night, neon lights, photorealistic",
        model="sdxl",
        width=1024,
        height=1024,
    )
    manager.assign_task(
        "video-agent",
        "analyze_video",
        description="Analyze a sample video clip",
        source="sample.mp4",
        analysis_type="scene_detection",
    )
    manager.assign_task(
        "workflow-agent",
        "export_comfyui",
        description="Export a sample workflow to ComfyUI format",
        workflow_name="sample_pipeline",
        steps=["load_image", "enhance", "caption", "export"],
        output_path="examples/sample_pipeline_comfyui.json",
    )

    # ----------------------------------------------------------------
    # Main monitoring loop
    # ----------------------------------------------------------------
    try:
        while True:
            time.sleep(30)
            print(f"\n[{time.strftime('%H:%M:%S')}] System heartbeat — agents active:")
            for agent in (text_agent, image_agent, video_agent, workflow_agent):
                print(f"  - {agent.agent_id}: {agent.status.value}")
            print(f"\n{manager.get_project_status_report()}")

    except KeyboardInterrupt:
        print("\n\n--- Shutting down all agents ---")
        for agent in (text_agent, image_agent, video_agent, workflow_agent):
            agent.stop()
            print(f"  Stopped: {agent.agent_id}")
        manager.stop()
        print("  Stopped: lonely_manager")
        print("\nSystem shutdown complete.")


if __name__ == "__main__":
    main()
