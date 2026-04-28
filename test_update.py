import re
import os

TASKS_FILE = "TASKS.md"
task_id = "1"

def test_update():
    with open(TASKS_FILE, "r", encoding="utf-8") as f:
        content = f.read()

    pattern = rf"^(\s*{re.escape(task_id)}\s+\[)([ ~])(\].*)$"
    new_content = []
    matches = 0
    for line in content.splitlines():
        if re.match(pattern, line):
            print(f"MATCHED: {line}")
            new_content.append(re.sub(pattern, r"\1x\3", line))
            matches += 1
        else:
            new_content.append(line)
            
    print(f"Total matches: {matches}")
    if matches > 0:
        with open("TASKS_TEST.md", "w", encoding="utf-8") as f:
            f.write("\n".join(new_content) + "\n")
        print("Wrote TASKS_TEST.md")

test_update()
