import os

path = 'crates/vaultwares-cli/src/main.rs'
with open(path, 'rb') as f:
    data = f.read()

# Try to find the corrupted pattern
# It might look like .ends_with('â€¦') or similar
# We want to replace it with .ends_with(\"…\")

# corrupted bytes for 'â€¦' are b'\xc3\xa2\xe2\x82\xac\xc2\xa6' in some encodings
# or b'\xe2\x80\xa6' for '…'

content = data.decode('utf-8', 'ignore')
if \"truncated.ends_with('\" in content:
    import re
    content = re.sub(r\"truncated\.ends_with\('.*?'\)\", 'truncated.ends_with(\"…\")', content)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
