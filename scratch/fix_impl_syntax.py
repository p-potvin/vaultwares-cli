import os
import re

def fix_file(path):
    with open(path, 'r', encoding='utf-8') as f:
        content = f.read()
    
    # Remove 'pub(crate) ' before 'impl '
    content = content.replace('pub(crate) impl ', 'impl ')
    # Also handle 'pub ' before 'impl '
    content = content.replace('pub impl ', 'impl ')
    
    with open(path, 'w', encoding='utf-8') as f:
        f.write(content)

def main():
    fix_file('crates/vaultwares-cli/src/app.rs')
    fix_file('crates/vaultwares-cli/src/args.rs')
    print("Fixed invalid pub(crate) impl syntax.")

if __name__ == '__main__':
    main()
