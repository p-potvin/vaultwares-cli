# Text Generation & Manipulation Agent

## Overview
This agent specializes in text generation, editing, and manipulation. It can:
- Generate captions, prompts, and answers from images, videos, or text
- Enhance and rewrite prompts for diffusion models (e.g., Stable Diffusion, SDXL)
- Perform VQA (Visual Question Answering) and batch operations
- Integrate into Python and ComfyUI workflows

## Skills
- Captioning images and videos in various styles (brief, detailed, tags, cinematic, sd_prompt)
- Enhancing prompts for generative models
- VQA and batch VQA
- Text-based workflow creation and conversion to ComfyUI/Diffusion formats
- Fluent interface for chaining text operations

## Task Types (dispatched by LonelyManager)
| Task ID | Description |
|---|---|
| `generate_text` | Generate text from a prompt with optional style |
| `generate_caption` | Caption an image or video source |
| `enhance_prompt` | Enhance a prompt for a target diffusion model |
| `vqa` | Answer a question about an image or video |
| `batch_vqa` | Perform VQA on a list of sources |
| `create_workflow` | Create a named text workflow definition |

## Example Usage
- Generate a detailed caption for an image
- Enhance a prompt for SDXL
- Create a text-based workflow and export to ComfyUI

## Integration
- Python: via `agents.TextAgent`
- Dispatch: `manager.assign_task("text-agent", "generate_caption", description="...", source="image.png")`

## Security & Style
- Follows VaultWares guidelines for privacy, security, and style
- No data is stored or shared externally
