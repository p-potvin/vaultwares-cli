import re

with open("crates/vaultwares-cli/src/main.rs", "r") as f:
    content = f.read()

# Extract CliAction
enum_cli_action = re.search(r"enum CliAction \{.*?\n\}", content, re.DOTALL)
if enum_cli_action:
    with open("crates/vaultwares-cli/src/args.rs", "w") as f:
        f.write("use crate::*;\nuse std::collections::BTreeSet;\nuse std::env;\nuse std::path::PathBuf;\n\n")
        f.write("pub " + enum_cli_action.group(0) + "\n\n")
    content = content.replace(enum_cli_action.group(0), "")

# Extract CliOutputFormat
enum_cli_output_format = re.search(r"enum CliOutputFormat \{.*?\n\}", content, re.DOTALL)
if enum_cli_output_format:
    with open("crates/vaultwares-cli/src/args.rs", "a") as f:
        f.write("pub " + enum_cli_output_format.group(0) + "\n\n")
    content = content.replace(enum_cli_output_format.group(0), "")

# Replace main content
with open("crates/vaultwares-cli/src/main.rs", "w") as f:
    f.write(content)
