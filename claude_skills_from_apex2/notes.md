# Notes on creating Claude skills from Apex2-Terminal-Bench-Agent

This file tracks the steps taken to implement Claude skills based on the Apex2-Terminal-Bench-Agent README.

- Created the `claude_skills_from_apex2` directory.
- Created this `notes.md` file.
- Read the Apex2-Terminal-Bench-Agent README.

## Key Ideas from Apex2 README

- **Predictive Intelligence:** Before execution, categorize the task, identify key files, and assess if multimodal analysis is needed. This allows for more targeted exploration.
- **Advanced Web Search:** Use a multi-round search pipeline with specific, low-frequency queries. Prioritize GitHub and StackOverflow, and extract Google's AI-generated summaries.
- **Heuristic Environment Observation:** Go beyond `ls` to check installed packages, folder structure, running processes, system state, and key file contents.
- **Deep Strategy Generation:** Prompt the LLM to surface its knowledge about the task, including alternative approaches, risk assessment, and common failure modes.
- **Strategy Synthesis:** Combine the results of the initial execution, web search, strategy generation, and environment observation to create an optimized context for subsequent execution.
- **Risk-Aware Category Prompting:** For high-risk tasks (e.g., ML, security), focus on managing high-consequence operations and common failure states.
- **Execution Optimization Suite:** A set of tools to handle common execution issues, such as heredoc management, session recovery, long-running task management, and recovery prompts for specific errors.
- **Predict → Explore → Synthesize → Execute:** This is the core architectural principle.
