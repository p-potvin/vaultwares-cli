import os
import re

def get_defs(path):
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Simple regex to find function and struct names
    fns = re.findall(r'fn\s+([a-z0-9_]+)', content)
    structs = re.findall(r'struct\s+([A-Za-z0-9_]+)', content)
    enums = re.findall(r'enum\s+([A-Za-z0-9_]+)', content)
    return set(fns + structs + enums)

def remove_duplicates(target_path, source_paths):
    all_source_defs = set()
    for sp in source_paths:
        all_source_defs.update(get_defs(sp))
        
    with open(target_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
        
    new_lines = []
    skip_until = -1
    for i, line in enumerate(lines):
        if i < skip_until:
            continue
            
        # Match start of fn, struct, etc.
        match = re.search(r'(fn|struct|enum)\s+([A-Za-z0-9_]+)', line)
        if match:
            name = match.group(2)
            if name in all_source_defs:
                # Find the end of this block (simple brace matching)
                if '{' in line:
                    braces = 1
                    j = i + 1
                    while j < len(lines) and braces > 0:
                        braces += lines[j].count('{')
                        braces -= lines[j].count('}')
                        j += 1
                    skip_until = j
                    print(f"Removing duplicate {name} from {target_path}")
                    continue
                else:
                    # Single line def or no braces
                    print(f"Removing duplicate {name} from {target_path}")
                    continue
        
        new_lines.append(line)
        
    with open(target_path, 'w', encoding='utf-8') as f:
        f.writelines(new_lines)

def main():
    session_mgr = 'crates/vaultwares-cli/src/session_mgr.rs'
    format_rs = 'crates/vaultwares-cli/src/format.rs'
    args_rs = 'crates/vaultwares-cli/src/args.rs'
    app_rs = 'crates/vaultwares-cli/src/app.rs'
    
    # Deduplicate app.rs against session_mgr and format
    remove_duplicates(app_rs, [session_mgr, format_rs])
    # Deduplicate args.rs against session_mgr and format
    remove_duplicates(args_rs, [session_mgr, format_rs])
    # Deduplicate app.rs against args.rs (items moved to args should be removed from app)
    remove_duplicates(app_rs, [args_rs])

if __name__ == '__main__':
    main()
