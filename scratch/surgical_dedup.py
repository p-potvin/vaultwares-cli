import os
import re

TO_REMOVE = [
    'SessionHandle', 'ManagedSessionSummary', 'sessions_dir', 'current_session_store',
    'new_cli_session', 'create_managed_session_handle', 'resolve_session_reference',
    'resolve_managed_session_path', 'list_managed_sessions', 'latest_managed_session',
    'load_session_reference', 'delete_managed_session', 'confirm_session_deletion',
    'render_session_list', 'format_session_modified_age', 'write_session_clear_backup',
    'session_clear_backup_path', 'resume_session', 'run_resume_command'
]

def remove_specific_duplicates(path):
    with open(path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
        
    new_lines = []
    skip_until = -1
    for i, line in enumerate(lines):
        if i < skip_until:
            continue
            
        # Match top-level items only
        match = re.match(r'^(pub\(crate\)\s+)?(fn|struct|enum)\s+([A-Za-z0-9_]+)', line)
        if match:
            name = match.group(3)
            if name in TO_REMOVE:
                # Find the start of the body {
                j = i
                while j < len(lines) and '{' not in lines[j]:
                    j += 1
                
                if j < len(lines) and '{' in lines[j]:
                    braces = 0
                    k = j
                    while k < len(lines):
                        braces += lines[k].count('{')
                        braces -= lines[k].count('}')
                        if braces == 0:
                            skip_until = k + 1
                            break
                        k += 1
                    print(f"Removing duplicate {name} from {path}")
                    continue
                else:
                    print(f"Removing duplicate {name} from {path} (no body found)")
                    skip_until = i + 1
                    continue
        
        new_lines.append(line)
        
    with open(path, 'w', encoding='utf-8') as f:
        f.writelines(new_lines)

def main():
    remove_specific_duplicates('crates/vaultwares-cli/src/app.rs')
    print("Surgical deduplication complete.")

if __name__ == '__main__':
    main()
