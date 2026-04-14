# Image Generation & Manipulation Agent

## Overview
This agent is specialized in image generation, editing, and manipulation. It can:
- Perform image transformations (resize, crop, rotate, sharpen, blur, etc.)
- Apply masks, inpainting, outpainting, and healing
- Generate and enhance prompts for image diffusion models
- Integrate into Python and ComfyUI workflows

## Skills
- Chainable image processing (ImageAgent)
- Mask creation and manipulation
- Inpainting/outpainting with or without model guidance
- Workflow creation and export to ComfyUI/Diffusion

## Task Types (dispatched by LonelyManager)
| Task ID | Description |
|---|---|
| `generate_image` | Generate an image from a prompt using a specified model |
| `edit_image` | Apply a list of operations to an existing image |
| `create_mask` | Create a segmentation mask for a region |
| `inpaint` | Inpaint a masked region with a prompt |
| `outpaint` | Extend an image beyond its borders |
| `create_workflow` | Create a named image workflow definition |
| `export_comfyui` | Export a workflow to ComfyUI JSON format |

## Example Usage
- Enhance a photo and generate a caption
- Inpaint a masked region and verify with a caption
- Build an image workflow and convert to ComfyUI

## Integration
- Python: via `agents.ImageAgent`
- Dispatch: `manager.assign_task("image-agent", "generate_image", description="...", prompt="a sunset", model="sdxl")`

## Security & Style
- Follows VaultWares guidelines for privacy, security, and style
- No data is stored or shared externally
