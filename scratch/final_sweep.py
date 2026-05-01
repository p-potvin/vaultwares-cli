import os
import re

def fix_visibility(path):
    with open(path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    new_lines = []
    in_struct = False
    in_trait_impl = False
    
    for line in lines:
        # Detect trait impl
        if re.match(r'^impl\s+.*?\s+for\s+', line):
            in_trait_impl = True
        elif line.startswith('}'):
            in_trait_impl = False
            in_struct = False
            
        # Detect struct start
        if re.match(r'^(pub\(crate\)\s+)?struct\s+.*\{', line):
            in_struct = True
            
        # 1. Top-level items
        if re.match(r'^(fn|struct|enum|type|trait|const) ', line):
            if not line.startswith('pub'):
                line = 'pub(crate) ' + line
        
        # 2. Struct fields
        if in_struct and re.match(r'^\s+[a-z0-9_]+:', line):
            if 'pub' not in line:
                line = line.replace('    ', '    pub(crate) ', 1)
        
        # 3. Methods (not in trait impl)
        if not in_trait_impl and re.match(r'^\s{4}fn ', line):
            if 'pub' not in line:
                line = line.replace('    fn ', '    pub(crate) fn ', 1)
                
        new_lines.append(line)
        
    with open(path, 'w', encoding='utf-8') as f:
        f.writelines(new_lines)

def restore_session_types():
    path = 'crates/vaultwares-cli/src/session_mgr.rs'
    with open(path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
        
    if any('struct SessionHandle' in l for l in lines):
        return
        
    insert_at = 1 # After use crate::*;
    new_types = [
        '\n',
        '#[derive(Debug, Clone)]\n',
        'pub(crate) struct SessionHandle {\n',
        '    pub(crate) id: String,\n',
        '    pub(crate) path: PathBuf,\n',
        '}\n',
        '\n',
        '#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]\n',
        'pub(crate) struct ManagedSessionSummary {\n',
        '    pub(crate) id: String,\n',
        '    pub(crate) path: PathBuf,\n',
        '    pub(crate) modified_epoch_millis: u128,\n',
        '    pub(crate) message_count: usize,\n',
        '    pub(crate) parent_session_id: Option<String>,\n',
        '    pub(crate) branch_name: Option<String>,\n',
        '}\n',
        '\n'
    ]
    lines = lines[:insert_at] + new_types + lines[insert_at:]
    with open(path, 'w', encoding='utf-8') as f:
        f.writelines(lines)

def main():
    restore_session_types()
    fix_visibility('crates/vaultwares-cli/src/app.rs')
    fix_visibility('crates/vaultwares-cli/src/args.rs')
    fix_visibility('crates/vaultwares-cli/src/session_mgr.rs')
    fix_visibility('crates/vaultwares-cli/src/format.rs')
    print("Visibility Sweep Complete.")

if __name__ == '__main__':
    main()
