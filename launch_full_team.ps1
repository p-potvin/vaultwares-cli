$manager = Start-Process -FilePath "python" -ArgumentList "vaultwares-agentciation\assign_tasks.py" -PassThru -NoNewWindow
Start-Sleep -Seconds 2

$agent1 = Start-Process -FilePath "python" -ArgumentList "-c `"import sys; import os; sys.path.append(os.path.join(os.getcwd(), 'vaultwares-agentciation')); from extrovert_agent import ExtrovertAgent; import time; agent = ExtrovertAgent('Security-Agent-1'); agent.start(); print('Security Agent 1 online.'); time.sleep(86400)`"" -PassThru -NoNewWindow
$agent2 = Start-Process -FilePath "python" -ArgumentList "-c `"import sys; import os; sys.path.append(os.path.join(os.getcwd(), 'vaultwares-agentciation')); from extrovert_agent import ExtrovertAgent; import time; agent = ExtrovertAgent('QA-Agent-2'); agent.start(); print('QA Agent 2 online.'); time.sleep(86400)`"" -PassThru -NoNewWindow
$agent3 = Start-Process -FilePath "python" -ArgumentList "-c `"import sys; import os; sys.path.append(os.path.join(os.getcwd(), 'vaultwares-agentciation')); from extrovert_agent import ExtrovertAgent; import time; agent = ExtrovertAgent('Dev-Agent-3'); agent.start(); print('Dev Agent 3 online.'); time.sleep(86400)`"" -PassThru -NoNewWindow

Write-Output "Full team deployed successfully! The Manager is reading from TASKS.md."
