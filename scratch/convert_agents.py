import tomllib
import os
import re

def convert_xml_to_markdown(text):
    lines = text.split('\n')
    new_lines = []
    
    for line in lines:
        match = re.match(r'^\s*<(\w+)>\s*$', line)
        if match:
            tag = match.group(1).replace('_', ' ').title()
            new_lines.append(f'## {tag}')
            continue
            
        match = re.match(r'^\s*</(\w+)>\s*$', line)
        if match:
            continue
            
        new_lines.append(line)
        
    return '\n'.join(new_lines)

def main():
    agents_dir = r'C:\Users\Administrator\.gemini\agents'
    skills_dir = r'C:\Users\Administrator\.gemini\skills'
    
    for filename in os.listdir(agents_dir):
        if filename.endswith('.toml'):
            path = os.path.join(agents_dir, filename)
            try:
                with open(path, 'rb') as f:
                    data = tomllib.load(f)
            except Exception as e:
                print(f"Error loading {filename}: {e}")
                continue
            
            agent_name = filename[:-5]
            title = data.get('name', agent_name).title()
            description = data.get('description', '')
            instructions = data.get('developer_instructions', '')
            
            content = convert_xml_to_markdown(instructions)
            
            md_content = f"# {title}\n\n"
            md_content += f"**Description**: {description}\n\n"
            md_content += content
            
            # Create skill folder
            skill_folder = os.path.join(skills_dir, agent_name)
            os.makedirs(skill_folder, exist_ok=True)
            
            skill_path = os.path.join(skill_folder, "SKILL.md")
            with open(skill_path, 'w', encoding='utf-8') as f:
                f.write(md_content)
                
            # Also keep a copy in agents dir as .md
            agent_md_path = os.path.join(agents_dir, f"{agent_name}.md")
            with open(agent_md_path, 'w', encoding='utf-8') as f:
                f.write(md_content)
                
            print(f"Converted {filename} to skill {agent_name} and agent .md")

if __name__ == "__main__":
    main()
