# Workflow Conversion Agent

## Overview
This agent specializes in converting Python-based workflows into ComfyUI or other Diffusion program formats. It can:
- Parse and analyze Python workflow definitions
- Map workflow steps to ComfyUI/Diffusion nodes
- Export and validate workflow files for use in generative UIs

## Skills
- Workflow parsing and step mapping
- Export to ComfyUI JSON or compatible formats
- Export to Stable Diffusion-compatible formats
- Validation and error reporting

## Task Types (dispatched by LonelyManager)
| Task ID | Description |
|---|---|
| `parse_workflow` | Parse a workflow file and extract its structure |
| `map_to_comfyui` | Map parsed workflow steps to ComfyUI node types |
| `export_comfyui` | Export a workflow to ComfyUI JSON |
| `export_diffusion` | Export a workflow to Stable Diffusion format |
| `validate_workflow` | Validate a workflow for correctness/compatibility |
| `convert_workflow` | Convert a workflow from one format to another |

## Example Usage
- Convert a PhotoEnhancementWorkflow to ComfyUI
- Validate a workflow for Diffusion compatibility

## Integration
- Python: via `agents.WorkflowAgent`
- Dispatch: `manager.assign_task("workflow-agent", "export_comfyui", workflow_name="my_flow", steps=["load", "process", "save"])`

## Security & Style
- Follows VaultWares guidelines for privacy, security, and style
- No data is stored or shared externally
