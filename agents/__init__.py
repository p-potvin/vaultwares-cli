"""
VaultWares Agents — specialized agent implementations.

All agents inherit from ExtrovertAgent and implement domain-specific
task handling. They connect to Redis and participate in the
LonelyManager heartbeat & dispatch network.
"""

import os
import sys

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))

from agents.text_agent import TextAgent
from agents.image_agent import ImageAgent
from agents.video_agent import VideoAgent
from agents.workflow_agent import WorkflowAgent

__all__ = [
    "TextAgent",
    "ImageAgent",
    "VideoAgent",
    "WorkflowAgent",
]
