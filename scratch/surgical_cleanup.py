import os
import re

TO_REMOVE = ['format_sandbox_report']

def remove_specific_duplicates(path):
    with open(path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
        
    new_lines = []
    skip_until = -1
    for i, line in enumerate(lines):
        if i < skip_until:
            continue
            
        match = re.match(r'^(pub\(crate\)\s+)?fn\s+([A-Za-z0-9_]+)', line)
        if match:
            name = match.group(2)
            if name in TO_REMOVE:
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
                    print(f"Removing {name} from {path}")
                    continue
        new_lines.append(line)
        
    with open(path, 'w', encoding='utf-8') as f:
        f.writelines(new_lines)

if __name__ == '__main__':
    remove_specific_duplicates('crates/vaultwares-cli/src/app.rs')
