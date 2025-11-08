# Claude Skills Inspired by Apex2-Terminal-Bench-Agent

This directory contains a set of Claude skills that implement the core architectural principles of the Apex2-Terminal-Bench-Agent, a system that achieved state-of-the-art performance on the Terminal Bench benchmark.

## Architectural Philosophy

The skills in this directory are designed to be used in a specific sequence, which mirrors the **Predict → Explore → Synthesize → Execute** workflow of the Apex2 agent.

1.  **Predict:** Use the `predictive_intelligence` skill to analyze the user's request and extract key information before you begin.
2.  **Explore:** Use the `advanced_web_search`, `environment_observation`, and `strategy_generation` skills to gather all the information you need to form a plan.
3.  **Synthesize:** Use the `strategy_synthesis` skill to combine all your findings into a single, optimized execution plan.
4.  **Execute:** Use the `execution_optimization` skill to guide you as you execute your plan in a robust and reliable manner.

## Skills

### 1. `predictive_intelligence`

*   **Objective:** To analyze the user's request and extract key information *before* beginning execution.
*   **Usage:** At the beginning of a new task, invoke this skill to categorize the task, identify key files, and assess the need for multimodal analysis.

### 2. `advanced_web_search`

*   **Objective:** To conduct a thorough and effective web search to gather the information needed to solve a task.
*   **Usage:** When you need to find information online, use this skill to guide your search process. It emphasizes multi-round searching, high-specificity queries, and the use of Google's AI Overview.

### 3. `environment_observation`

*   **Objective:** To gather a comprehensive understanding of the current environment.
*   **Usage:** When you need to understand the state of the system, use this skill to run a checklist of commands that go beyond a simple `ls`.

### 4. `strategy_generation`

*   **Objective:** To move beyond a single, reactive solution and instead develop a portfolio of well-reasoned strategies.
*   **Usage:** After you have a good understanding of the task, use this skill to generate multiple solution strategies, assess their risks, and anticipate common failures.

### 5. `strategy_synthesis`

*   **Objective:** To synthesize the information gathered from all other skills into a single, coherent, and optimized execution plan.
*   **Usage:** This is the final step in the planning process. Use this skill to create a detailed, step-by-step execution plan that you will follow.

### 6. `execution_optimization`

*   **Objective:** To ensure that commands are executed in a robust and reliable manner.
*   **Usage:** As you execute your plan, use this skill as a guide for best practices, including `heredoc` usage, error recovery, and final validation.
