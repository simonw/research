# Predictive Intelligence Skill

**Objective:** To analyze the user's request and extract key information *before* beginning execution. This helps to form a more accurate and efficient plan.

**Instructions:**

When you invoke this skill, you must answer the following questions to the best of your ability. Frame your answers as a preliminary analysis.

1.  **Task Categorization:**
    *   What is the primary goal of this task? (e.g., install a package, debug a script, write a new feature, analyze data).
    *   What is the risk profile of this task? (e.g., low-risk, like reading a file; high-risk, like deleting files or modifying a production environment).
    *   What is the most likely category for this task? (e.g., "File System Operations", "Code Development", "Data Analysis", "Security", "Machine Learning").

2.  **Key File Identification:**
    *   Are there any specific file or directory names mentioned in the user's request?
    *   If so, list them.
    *   What is the likely purpose of each of these files?

3.  **Multimodal Assessment:**
    *   Does the user's request reference any images, videos, or other non-textual content?
    *   Is it likely that you will need to analyze images or other media to successfully complete this task?

**Example Usage:**

**User Request:** "There's a bug in `main.py` that's causing a `FileNotFoundError`. I think it has something to do with `requirements.txt`. Can you fix it?"

**Your Preliminary Analysis:**

*   **Task Categorization:**
    *   **Goal:** Debug and fix a `FileNotFoundError` in a Python script.
    *   **Risk Profile:** Medium-risk. The task involves modifying code, which could introduce new bugs.
    *   **Category:** "Code Development".
*   **Key File Identification:**
    *   **Files:** `main.py`, `requirements.txt`.
    *   **Purpose:** `main.py` is likely the main application file, and `requirements.txt` probably lists the project's dependencies. The error might be caused by a missing dependency.
*   **Multimodal Assessment:**
    *   **Media Referenced:** No.
    *   **Analysis Needed:** No, this is a code-based task.
