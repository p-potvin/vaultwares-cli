import os
import re

def fix_file(path):
    with open(path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
    
    new_lines = []
    in_struct = False
    for line in lines:
        # Make all private methods in impl LiveCli pub(crate)
        # Actually, let's just make ALL methods in ALL impl blocks pub(crate) if they aren't pub
        if re.match(r'^\s{4}fn ', line) and 'pub' not in line:
            new_lines.append(line.replace('    fn ', '    pub(crate) fn ', 1))
        # Make all private fields in structs pub(crate)
        elif re.match(r'^\s{4}[a-z_]+:', line) and 'pub' not in line:
            new_lines.append('    pub(crate) ' + line.lstrip())
        # Make all top-level fns pub(crate)
        elif re.match(r'^fn ', line):
            new_lines.append('pub(crate) ' + line)
        # Make all top-level structs/enums/etc pub(crate)
        elif re.match(r'^(struct|enum|type|trait) ', line):
            new_lines.append('pub(crate) ' + line)
        else:
            new_lines.append(line)
            
    with open(path, 'w', encoding='utf-8') as f:
        f.writelines(new_lines)

def main():
    fix_file('crates/vaultwares-cli/src/app.rs')
    fix_file('crates/vaultwares-cli/src/args.rs')
    print("Visibility fix applied to app.rs and args.rs")

if __name__ == '__main__':
    main()
