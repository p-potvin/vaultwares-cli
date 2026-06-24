# VaultWares CLI Command Registry
# Defines all executable commands and maps them to their respective workspace script paths.

$RepoRoot = "C:\Users\Administrator\Desktop\Github Repos"

$Commands = [ordered]@{
    # Builds & Workspace Sync
    "sync" = @{
        ScriptPath  = "$RepoRoot\.builds\batch-phase5-sync.ps1"
        Description = "Run bulk Git pull and workspace synchronization across repository layers."
        Category    = "Workspace Management"
    }
    "shortcuts" = @{
        ScriptPath  = "$RepoRoot\.builds\create-shortcuts.ps1"
        Description = "Install or refresh Desktop shortcuts for standard tool configurations."
        Category    = "Workspace Management"
    }
    "rebuild" = @{
        ScriptPath  = "$RepoRoot\.builds\rebuild-all.ps1"
        Description = "Clean and rebuild python packages, node modules, and dependencies."
        Category    = "Workspace Management"
    }
    "recreate-venvs" = @{
        ScriptPath  = "$RepoRoot\.builds\recreate-venvs.ps1"
        Description = "Reinitialize virtual environments (.venv) across workspaces."
        Category    = "Workspace Management"
        Destructive = $true
    }
    "git-pull-all" = @{
        ScriptPath  = "$RepoRoot\.extras\github_pull_all.ps1"
        Description = "Iterate through workspace and pull updates for submodules and branches."
        Category    = "Git Automation"
    }
    "git-auto" = @{
        ScriptPath  = "$RepoRoot\.extras\github_automation_script.ps1"
        Description = "Execute repository checks and send email alerts on remote changes."
        Category    = "Git Automation"
    }

    # Agent Ledger
    "archive-ledger" = @{
        ScriptPath  = "$RepoRoot\agent-ledger\scripts\archive-old-ledger-entries.ps1"
        Description = "Archive historical ledger entries to optimize performance."
        Category    = "Agent Ledger"
        Destructive = $true
    }
    "record-change" = @{
        ScriptPath  = "$RepoRoot\agent-ledger\scripts\record-agent-change.ps1"
        Description = "Log an agent transaction or changes to the Shared Agent Ledger database."
        Category    = "Agent Ledger"
    }
    "render-ledger" = @{
        ScriptPath  = "$RepoRoot\agent-ledger\scripts\render-agent-ledger.ps1"
        Description = "Generate detailed HTML/Markdown reports of agent activities."
        Category    = "Agent Ledger"
    }
    "render-impact" = @{
        ScriptPath  = "$RepoRoot\agent-ledger\scripts\render-work-impact.ps1"
        Description = "Compile work metrics and update visual workspace progression indicators."
        Category    = "Agent Ledger"
    }
    "sync-ledger" = @{
        ScriptPath  = "$RepoRoot\agent-ledger\scripts\sync-agent-ledger.ps1"
        Description = "Sync local ledger records to central database endpoints."
        Category    = "Agent Ledger"
    }
    "start-tracker" = @{
        ScriptPath  = "$RepoRoot\agent-ledger\scripts\start-input-tracker.ps1"
        Description = "Launch the keyboard/mouse idle telemetry service in the background."
        Category    = "Agent Ledger"
    }

    # Health Ledger & Monitoring
    "health-alarm" = @{
        ScriptPath  = "$RepoRoot\health-ledger\ops\windows\start-health-ledger-alarm.ps1"
        Description = "Start the telemetry status alert service using local environment configuration."
        Category    = "System Monitoring"
    }
    "health-probe" = @{
        ScriptPath  = "$RepoRoot\health-ledger\ops\windows\start-health-ledger-probe.ps1"
        Description = "Run immediate ping checks and log network reachability for all nodes."
        Category    = "System Monitoring"
    }
    "pg-watchdog" = @{
        ScriptPath  = "$RepoRoot\health-ledger\scripts\pg-watchdog.ps1"
        Description = "Launch background monitor to auto-restart Postgres service if hung."
        Category    = "System Monitoring"
    }
    "drill-kill-pg" = @{
        ScriptPath  = "$RepoRoot\health-ledger\scripts\drill-kill-pg.ps1"
        Description = "Forcefully close active Postgres database connections and clean locks."
        Category    = "System Monitoring"
        Destructive = $true
    }
    "test-alarm" = @{
        ScriptPath  = "$RepoRoot\health-ledger\ops\windows\test-alarm.ps1"
        Description = "Dispatch simulated SMTP notification message for alert verification."
        Category    = "System Monitoring"
    }

    # Media & Ingestion Pipelines
    "move-videos" = @{
        ScriptPath  = "$RepoRoot\python-scripts\scripts\move_videos.ps1"
        Description = "Move all video files from subfolders recursively to the root directory."
        Category    = "Media Pipeline"
        Destructive = $true
    }
    "unzip-dedupe" = @{
        ScriptPath  = "$RepoRoot\python-zipper\.extras\unzip_dedupe.ps1"
        Description = "Decompress volumes and delete matching hash duplicates automatically."
        Category    = "Media Pipeline"
        Destructive = $true
    }
    "upscale-test" = @{
        ScriptPath  = "$RepoRoot\python-zipper\.extras\upscale.ps1"
        Description = "Execute standalone upscaling test using local PyTorch/Real-ESRGAN dependencies."
        Category    = "Media Pipeline"
    }
    "telegram-pipeline" = @{
        ScriptPath  = "$RepoRoot\python-zipper\telegram\run_pipeline_with_notification.ps1"
        Description = "Execute telethon link resolver pipeline with system notifications."
        Category    = "Media Pipeline"
    }

    # Vault Explorer Media Processors
    "benchmark-asr" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-AsrBenchmark.ps1"
        Description = "Measure speech-to-text benchmark performance using local Whisper models."
        Category    = "Vault Explorer"
    }
    "audio-normalize" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-AudioNormalization.ps1"
        Description = "Run loudness normalization package recursively on video/audio folders."
        Category    = "Vault Explorer"
    }
    "imagemagick-enhance" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-ImageMagickEnhancement.ps1"
        Description = "Enhance cover artwork/thumbnails using ImageMagick filters."
        Category    = "Vault Explorer"
    }
    "livestream-translator" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-LivestreamTranslator.ps1"
        Description = "Translate spoken audio tracks from RTMP/HTTP livestreams in real-time."
        Category    = "Vault Explorer"
    }
    "preview-generator" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-PreviewGenerator.ps1"
        Description = "Compile preview hover-grids, thumbnails, and contact sheets."
        Category    = "Vault Explorer"
    }
    "realesrgan-enhance" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-RealesrganImageEnhancement.ps1"
        Description = "Upscale thumbnail assets using PyTorch CUDA Real-ESRGAN."
        Category    = "Vault Explorer"
    }
    "rtx-vsr" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-RtxVsrEnhancement.ps1"
        Description = "Render video upscales using NVIDIA RTX Video Super Resolution."
        Category    = "Vault Explorer"
    }
    "subtitles" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-SubtitleGeneration.ps1"
        Description = "Generate SRT/VTT subtitles from spoken audio using speech models."
        Category    = "Vault Explorer"
    }
    "thumb-enhance" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-ThumbnailEnhancement.ps1"
        Description = "Optimize cover pages and preview layouts."
        Category    = "Vault Explorer"
    }
    "translate-spoken" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\scripts\pwsh\Start-Translation.ps1"
        Description = "Recursively translate spoken dialog in video files via Whisper/TTS."
        Category    = "Vault Explorer"
    }
    "stream-translator" = @{
        ScriptPath  = "$RepoRoot\vault-explorer\powershell\Start-StreamTranslator.ps1"
        Description = "Run Stream spoken translation for active HTTPS audio/video streams."
        Category    = "Vault Explorer"
    }

    # SSL & Local Servers
    "generate-certs" = @{
        ScriptPath  = "$RepoRoot\vaultwares-website\generate_local_tls_certs.ps1"
        Description = "Generate local dev SSL certificates using mkcert."
        Category    = "Security & Server"
    }
    "start-https" = @{
        ScriptPath  = "$RepoRoot\vaultwares-website\start_https_dev.ps1"
        Description = "Spin up local front-end Astro development server under HTTPS."
        Category    = "Security & Server"
    }
    "build-gradio" = @{
        ScriptPath  = "$RepoRoot\vaultwares-api\build_gradio_app.ps1"
        Description = "Compile python Gradio application using PyInstaller standalone builder."
        Category    = "Security & Server"
    }
    "launch-team" = @{
        ScriptPath  = "$RepoRoot\vaultwares-cli\launch_full_team.ps1"
        Description = "Dispatch team of supervisor and worker AI agents."
        Category    = "Workspace Management"
    }
    "deploy-dispatch" = @{
        ScriptPath  = "$RepoRoot\vaultwares-dispatch\deploy_vaultwares_dispatch.ps1"
        Description = "Bootstrap VaultWares dispatch server directories."
        Category    = "Workspace Management"
    }

    # Corporate Protocols & Instruction sync
    "sync-instructions" = @{
        ScriptPath  = "$RepoRoot\vaultwares-docs\scripts\sync-global-instructions.ps1"
        Description = "Propagate global system prompts and rules across all repositories."
        Category    = "Corporate Standards"
    }
    "sync-skills" = @{
        ScriptPath  = "$RepoRoot\vaultwares-docs\scripts\sync-global-skills.ps1"
        Description = "Sync global assistant skills files to local repo locations."
        Category    = "Corporate Standards"
    }
    "validate-protocols" = @{
        ScriptPath  = "$RepoRoot\vaultwares-docs\scripts\validate-assistant-protocols.ps1"
        Description = "Validate repo system instruction formatting and metadata compliance."
        Category    = "Corporate Standards"
    }

    # Studio and usd Playgrounds
    "download-colmap" = @{
        ScriptPath  = "$RepoRoot\vaultwares-studio\download_colmap.ps1"
        Description = "Download portable COLMAP 3D reconstruction tool."
        Category    = "USD Studio"
    }
    "setup-usd" = @{
        ScriptPath  = "$RepoRoot\vaultwares-studio\setup_env.ps1"
        Description = "Bootstrap PyTorch and CUDA workspace for USD Digital Twin Playground."
        Category    = "USD Studio"
    }
    "build-worker-img" = @{
        ScriptPath  = "$RepoRoot\vaultwares-studio\tools\build_worker_image.ps1"
        Description = "Build and push docker worker containers."
        Category    = "USD Studio"
    }
    "build-demo-exe" = @{
        ScriptPath  = "$RepoRoot\vaultwares-studio\build_demo_exe.ps1"
        Description = "Compile standalone USD Studio Demo executable."
        Category    = "USD Studio"
    }
    "distribute-secrets" = @{
        ScriptPath  = "$RepoRoot\vw-jira-sync\scripts\scheduled-distribute-secrets.ps1"
        Description = "Rotate and distribute GH_TOKEN and JIRA_TOKEN secrets to repositories."
        Category    = "Security & Server"
    }
}

return $Commands
