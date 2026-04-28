import re
import os

TASKS_FILE = "TASKS.md"

def parse_tasks():
    if not os.path.exists(TASKS_FILE):
        print("TASKS.md NOT FOUND")
        return []
        
    tasks = []
    with open(TASKS_FILE, "r", encoding="utf-8") as f:
        lines = f.readlines()
        
    for i, line in enumerate(lines):
        stripped = line.strip()
        print(f"Line {i}: '{stripped}'")
        main_match = re.match(r"^(\d+)\s+\[(.*?)\]\s+(.*)$", stripped)
        if main_match:
            print(f"  MATCH MAIN: ID={main_match.group(1)} STATUS='{main_match.group(2)}' TITLE='{main_match.group(3)}'")
            tasks.append({
                "id": main_match.group(1),
                "status": main_match.group(2).strip(),
                "title": main_match.group(3).strip(),
                "type": "main"
            })
            continue
            
        sub_match = re.match(r"^\s+(\d+[a-z]+)\s+\[(.*?)\]\s+(.*)$", line)
        if sub_match:
            print(f"  MATCH SUB: ID={sub_match.group(1)} STATUS='{sub_match.group(2)}' TITLE='{sub_match.group(3)}'")
            tasks.append({
                "id": sub_match.group(1),
                "status": sub_match.group(2).strip(),
                "title": sub_match.group(3).strip(),
                "type": "subtask"
            })
            
    return tasks

parse_tasks()
