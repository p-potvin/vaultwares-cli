import time
import os
import sys

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))
from vaultwares_agentciation.extrovert_agent import ExtrovertAgent
from vaultwares_agentciation.enums import AgentStatus


class VideoAgent(ExtrovertAgent):
    """
    Video Generation & Manipulation Agent.

    Specializes in:
    - Video sampling, trimming, resizing, and frame-level processing
    - Per-frame effects, overlays, and stabilization
    - Video description and per-frame captioning
    - Workflow creation and export to ComfyUI/Diffusion formats

    Inherits the full Extrovert personality: heartbeat every 5 seconds,
    status broadcast every minute, socialization on every user interaction.
    """

    AGENT_TYPE = "video"
    SKILLS = [
        "video_trimming",
        "video_resizing",
        "frame_sampling",
        "per_frame_effects",
        "video_captioning",
        "video_analysis",
        "workflow_creation",
        "comfyui_export",
    ]

    def __init__(
        self,
        agent_id: str = "video-agent",
        channel: str = "tasks",
        redis_host: str = "localhost",
        redis_port: int = 6379,
        redis_db: int = 0,
    ):
        super().__init__(agent_id, channel, redis_host, redis_port, redis_db)

    # ------------------------------------------------------------------
    # Task Execution
    # ------------------------------------------------------------------

    def _perform_task(self, task: str, details: dict):
        """Execute a video processing task based on the task identifier."""
        print(f"🎬 [{self.agent_id}] Executing video task: {task}")

        handlers = {
            "trim_video": self._trim_video,
            "resize_video": self._resize_video,
            "sample_frames": self._sample_frames,
            "apply_effects": self._apply_effects,
            "generate_caption": self._generate_video_caption,
            "analyze_video": self._analyze_video,
            "create_workflow": self._create_video_workflow,
            "export_comfyui": self._export_comfyui,
        }

        handler = handlers.get(task)
        if handler:
            handler(details)
        else:
            print(f"⚠️  [{self.agent_id}] Unknown video task: {task}. Logging and continuing.")
            self._log_unknown_task(task, details)

    def _trim_video(self, details: dict):
        """Trim a video to a specified time range."""
        source = details.get("source", "unknown")
        start = details.get("start_time", 0)
        end = details.get("end_time", None)
        print(f"✂️  [{self.agent_id}] Trimming video | source={source} | {start}s → {end}s")
        time.sleep(1)
        self._publish_result("trim_video", f"Video '{source}' trimmed from {start}s to {end}s")

    def _resize_video(self, details: dict):
        """Resize a video to specified dimensions."""
        source = details.get("source", "unknown")
        width = details.get("width", 1280)
        height = details.get("height", 720)
        print(f"📐 [{self.agent_id}] Resizing video | source={source} | {width}x{height}")
        time.sleep(1)
        self._publish_result("resize_video", f"Video '{source}' resized to {width}x{height}")

    def _sample_frames(self, details: dict):
        """Extract a set of frames from a video at specified intervals."""
        source = details.get("source", "unknown")
        fps = details.get("fps", 1)
        count = details.get("count", None)
        print(f"🎞️  [{self.agent_id}] Sampling frames | source={source} | fps={fps} | count={count}")
        time.sleep(1)
        frames_sampled = count if count else "all"
        self._publish_result("sample_frames", f"Sampled {frames_sampled} frames from '{source}' at {fps} fps")

    def _apply_effects(self, details: dict):
        """Apply per-frame effects and overlays to a video."""
        source = details.get("source", "unknown")
        effects = details.get("effects", [])
        print(f"✨ [{self.agent_id}] Applying effects | source={source} | effects={effects}")
        for effect in effects:
            time.sleep(0.3)
            print(f"  ✅ Applied effect: {effect}")
        self._publish_result("apply_effects", f"Applied {len(effects)} effects to '{source}'")

    def _generate_video_caption(self, details: dict):
        """Generate a caption or summary for a video."""
        source = details.get("source", "unknown")
        caption_style = details.get("caption_style", "detailed")
        print(f"💬 [{self.agent_id}] Generating video caption | source={source} | style={caption_style}")
        time.sleep(1)
        result = f"[Video caption for '{source}' in '{caption_style}' style]"
        self._publish_result("generate_caption", result)

    def _analyze_video(self, details: dict):
        """Perform analysis on a video (scene detection, object tracking, etc.)."""
        source = details.get("source", "unknown")
        analysis_type = details.get("analysis_type", "general")
        print(f"🔍 [{self.agent_id}] Analyzing video | source={source} | type={analysis_type}")
        time.sleep(2)
        result = f"[Video analysis '{analysis_type}' complete for '{source}']"
        self._publish_result("analyze_video", result)

    def _create_video_workflow(self, details: dict):
        """Create a video processing workflow definition."""
        workflow_name = details.get("name", "unnamed_workflow")
        steps = details.get("steps", [])
        print(f"🔧 [{self.agent_id}] Creating video workflow: {workflow_name} ({len(steps)} steps)")
        time.sleep(1)
        self._publish_result("create_workflow", f"Video workflow '{workflow_name}' created with {len(steps)} steps")

    def _export_comfyui(self, details: dict):
        """Export a workflow to ComfyUI JSON format."""
        workflow_name = details.get("workflow_name", "unnamed")
        output_path = details.get("output_path", f"{workflow_name}.json")
        print(f"📦 [{self.agent_id}] Exporting to ComfyUI: {workflow_name} → {output_path}")
        time.sleep(1)
        self._publish_result("export_comfyui", f"ComfyUI export complete: {output_path}")

    def _log_unknown_task(self, task: str, details: dict):
        """Log an unrecognized task for debugging."""
        print(f"📋 [{self.agent_id}] Unknown task '{task}' — details: {details}")

    def _publish_result(self, task: str, result: str):
        """Publish a task result back to the Redis channel."""
        self.coordinator.publish(
            "RESULT",
            task,
            {
                "agent": self.agent_id,
                "task": task,
                "result": result,
            },
        )
        print(f"📤 [{self.agent_id}] Result published for task '{task}'")
