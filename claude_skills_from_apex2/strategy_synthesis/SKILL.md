# Strategy Synthesis Skill

**Objective:** To synthesize the information gathered from all other skills into a single, coherent, and optimized execution plan. This is the final planning step before execution begins.

**Instructions:**

When you invoke this skill, you must create a final execution plan that integrates the findings from the following skills:

*   `predictive_intelligence`
*   `advanced_web_search`
*   `environment_observation`
*   `strategy_generation`

Your final plan should be structured as follows:

**1. Consolidated Context:**
*   **Task Summary:** Briefly re-state the user's goal.
*   **Key Files:** List the key files and their purpose.
*   **Environment State:** Summarize the most relevant findings from your environment observation.
*   **Web Research:** Briefly summarize the most promising solutions or insights from your web search.

**2. Selected Strategy:**
*   Based on all the information you have gathered, which of the strategies you generated is the most likely to succeed?
*   Justify your choice. Why is this strategy superior to the alternatives?

**3. Step-by-Step Execution Plan:**
*   Provide a detailed, numbered list of the exact commands you will execute to implement the selected strategy.
*   **Clarity is critical:** Each step should be a single, unambiguous command.
*   Include comments where necessary to explain the purpose of a command.
*   Use `heredoc` for creating or modifying files.

**4. Pre-computation and Verification:**
*   Before each step, state the expected outcome of the command.
*   After each step, describe how you will verify that the command was successful.

**Example Usage:**

**Task:** "Fix the `FileNotFoundError` in `main.py`."

**Execution Plan:**

*   **Consolidated Context:**
    *   **Task Summary:** The user wants to fix a `FileNotFoundError` in `main.py`, which is likely caused by a missing dependency listed in `requirements.txt`.
    *   **Key Files:** `main.py` (the application script), `requirements.txt` (the dependency list).
    *   **Environment State:** The `requests` library, which is listed in `requirements.txt`, is not installed in the current Python environment.
    *   **Web Research:** StackOverflow confirms that a `FileNotFoundError` can occur if a program tries to access a file that is expected to be created by a dependency that is not installed.

*   **Selected Strategy:**
    *   The best strategy is to install the missing dependencies from `requirements.txt`. This is a low-risk, high-reward action that directly addresses the likely root cause of the problem.

*   **Step-by-Step Execution Plan:**
    1.  **Command:** `pip install -r requirements.txt`
        *   **Expected Outcome:** The `requests` library will be installed.
        *   **Verification:** Run `pip list | grep requests` and expect to see `requests` in the output.
    2.  **Command:** `python main.py`
        *   **Expected Outcome:** The script will run without a `FileNotFoundError`.
        *   **Verification:** Check the script's output for the expected result.
