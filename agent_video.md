# Video Generation & Manipulation Agent

## Overview
This agent is specialized in video generation, editing, and manipulation. It can:
- Sample, trim, resize, and process video frames
- Apply per-frame effects, overlays, and stabilization
- Generate video descriptions and per-frame captions
- Integrate into Python and ComfyUI workflows

## Skills
- Chainable video processing (VideoAgent)
- Frame sampling, trimming, resizing, and effect application
- Video analysis and captioning
- Workflow creation and export to ComfyUI/Diffusion

## Task Types (dispatched by LonelyManager)
| Task ID | Description |
|---|---|
| `trim_video` | Trim a video to a start/end time range |
| `resize_video` | Resize a video to specified dimensions |
| `sample_frames` | Extract frames at a given fps or count |
| `apply_effects` | Apply per-frame effects to a video |
| `generate_caption` | Generate a caption or summary for a video |
| `analyze_video` | Perform scene detection, object tracking, etc. |
| `create_workflow` | Create a named video workflow definition |
| `export_comfyui` | Export a workflow to ComfyUI JSON format |

## Example Usage
- Analyze a video and generate a summary
- Edit a video (trim, resize, effect) and export
- Build a video workflow and convert to ComfyUI

## Integration
- Python: via `agents.VideoAgent`
- Dispatch: `manager.assign_task("video-agent", "trim_video", description="...", source="clip.mp4", start_time=10, end_time=30)`

## Security & Style
- Follows VaultWares guidelines for privacy, security, and style
- No data is stored or shared externally
