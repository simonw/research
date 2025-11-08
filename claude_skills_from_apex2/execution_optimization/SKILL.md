# Execution Optimization Skill

**Objective:** To ensure that commands are executed in a robust and reliable manner. This skill provides a set of best practices for common execution challenges.

**Instructions:**

When you are executing your plan, you must adhere to the following principles:

**1. Heredoc Management:**
*   **Always** use `heredoc` to create or overwrite files. This is the most reliable way to handle multi-line content.
*   Use the `EOF` marker and single quotes to prevent shell expansion: `cat << 'EOF' > filename.txt`
*   **Automatic Repair:** If a `heredoc` command fails, check for common errors like incorrect indentation or unescaped special characters.

**2. Session Recovery:**
*   If the terminal becomes unresponsive or you encounter a `tmux` error, do not panic.
*   Attempt to re-attach to the `tmux` session. If that fails, create a new session and use your plan to get back to the current step.

**3. Long-Running Task Management:**
*   For commands that are expected to take more than 30 seconds (e.g., training a machine learning model, running a large test suite), you must:
    *   Run the command in the background using `&`.
    *   Redirect the output to a log file: `long_running_command > command.log 2>&1 &`
    *   Monitor the log file for progress: `tail -f command.log`

**4. Recovery Prompts:**
*   If a command fails, do not immediately try a different strategy. First, attempt to diagnose and fix the specific error.
*   Consult your `strategy_generation` document for common failures and their remediation steps.
*   **Common Error Types:**
    *   **Syntax Error:** Carefully review the command for typos or incorrect syntax.
    *   **Import Error:** The necessary library is likely not installed. Use `pip install` or `npm install` to fix it.
    *   **File Not Found:** The file or directory does not exist. Use `ls` or `find` to verify the path.
    *   **Permission Denied:** You do not have the necessary permissions to read, write, or execute the file. Use `ls -l` to check permissions and `chmod` or `sudo` if necessary (and appropriate).
    *   **Connection Timeout:** A network service is unavailable. Check if the service is running and if there are any firewall rules blocking the connection.

**5. Final Validation Tuning:**
*   Before marking a task as complete, you must perform a final validation to ensure that the user's request has been fully satisfied.
*   **Validation Checklist:**
    *   Did the primary command(s) execute without any errors?
    *   Does the output of the script or program match the user's expectations?
    *   Have all the requirements of the task been met?
    *   Are there any lingering issues or side effects?
*   **Do not prematurely complete the task.** If you have any doubts, perform additional verification steps.
