import os
import re

def fix_trait_impls(path):
    with open(path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
        
    new_lines = []
    in_trait_impl = False
    for line in lines:
        if re.match(r'^impl\s+.*?\s+for\s+', line):
            in_trait_impl = True
            
        if in_trait_impl:
            if line.startswith('}'):
                in_trait_impl = False
            elif 'pub(crate) fn ' in line:
                line = line.replace('pub(crate) fn ', 'fn ')
                
        new_lines.append(line)
        
    with open(path, 'w', encoding='utf-8') as f:
        f.writelines(new_lines)

def main():
    fix_trait_impls('crates/vaultwares-cli/src/app.rs')
    fix_trait_impls('crates/vaultwares-cli/src/args.rs')
    print("Fixed trait impl visibility.")

if __name__ == '__main__':
    main()
