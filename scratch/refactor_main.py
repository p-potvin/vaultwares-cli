import os
import re

def process_visibility(lines, is_app=True):
    processed = []
    in_struct = False
    for line in lines:
        # 1. Top-level items
        if re.match(r'^(fn|struct|enum|type|trait|const) ', line):
            if not line.startswith('pub'):
                line = 'pub(crate) ' + line
        
        # 2. Struct fields (inside struct definition)
        if in_struct:
            if line.strip() == '}':
                in_struct = False
            elif re.match(r'^\s+[a-z_]+:', line):
                if 'pub' not in line:
                    line = line.replace('    ', '    pub(crate) ', 1)
        
        if 'struct ' in line and '{' in line and not line.strip().startswith('//'):
            in_struct = True
            
        # 3. Methods in impl blocks (indented 4 spaces)
        if is_app and re.match(r'^\s{4}fn ', line):
            if 'pub' not in line:
                line = line.replace('    fn ', '    pub(crate) fn ', 1)
                
        processed.append(line)
    return processed

def main():
    main_path = 'crates/vaultwares-cli/src/main.rs'
    args_path = 'crates/vaultwares-cli/src/args.rs'
    app_path = 'crates/vaultwares-cli/src/app.rs'
    
    with open(main_path, 'r', encoding='utf-8') as f:
        orig_lines = f.readlines()
        
    parse_args_code = orig_lines[325:2141]
    live_cli_code = orig_lines[2294:8484]
    
    # Keep 1-325, then 2142-2294, then 8485-end
    new_main_lines = orig_lines[:325] + orig_lines[2141:2294] + orig_lines[8484:]
    
    processed_args = process_visibility(parse_args_code, is_app=False)
    processed_app = process_visibility(live_cli_code, is_app=True)
    
    with open(args_path, 'a', encoding='utf-8') as f:
        f.write('\n// --- Extracted from main.rs ---\n')
        f.writelines(processed_args)
        
    with open(app_path, 'a', encoding='utf-8') as f:
        f.write('\n// --- Extracted from main.rs ---\n')
        f.writelines(processed_app)
        
    with open(main_path, 'w', encoding='utf-8') as f:
        f.writelines(new_main_lines)
        
    print("Refactor complete with improved visibility handling.")

if __name__ == '__main__':
    main()
