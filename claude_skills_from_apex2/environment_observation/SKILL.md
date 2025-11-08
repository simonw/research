# Environment Observation Skill

**Objective:** To gather a comprehensive understanding of the current environment. This skill goes beyond a simple `ls` to build a detailed picture of the system's state, which is crucial for effective diagnosis and planning.

**Instructions:**

When you invoke this skill, you should execute a series of commands to investigate the environment. Use your judgment to select the most relevant commands for the current task. **Do not execute all of these commands blindly.**

**1. File System Structure:**
*   List the contents of the current directory, including hidden files: `ls -la`
*   Recursively list the contents of the current directory: `ls -R`
*   Find all files of a specific type (e.g., `.py`, `.js`, `.json`): `find . -type f -name "*.py"`
*   Display the directory structure in a tree-like format: `tree -L 2` (if `tree` is installed)

**2. Installed Packages and Dependencies:**
*   For Python projects, list installed packages: `pip list` or `pip freeze`
*   For Node.js projects, list installed packages: `npm list`
*   Check the contents of dependency files: `cat requirements.txt`, `cat package.json`

**3. Running Processes:**
*   List all running processes: `ps aux`
*   Filter for specific processes (e.g., `python`, `node`, `java`): `ps aux | grep python`
*   List running Docker containers: `docker ps -a`

**4. System State:**
*   Check disk space usage: `df -h`
*   Check memory usage: `free -m`
*   Check network connections: `netstat -tuln`

**5. Key File Contents:**
*   Based on the `predictive_intelligence` skill, read the contents of any identified key files.
    *   Example: `cat main.py`, `cat config.json`

**Synthesize Findings:**

After running the relevant commands, create a summary of your findings. This summary should highlight any information that is particularly relevant to the current task.

**Example Usage:**

**Task:** "The Flask application in this directory is failing to start. Can you figure out why?"

**Observation Process:**

1.  `ls -la`: Look for `app.py`, `requirements.txt`, and a `.flaskenv` file.
2.  `cat requirements.txt`: Check if `Flask` is listed as a dependency.
3.  `pip list | grep Flask`: Verify that Flask is actually installed in the environment.
4.  `ps aux | grep python`: Check if a Python process is already running, which might be holding a port.
5.  `cat app.py`: Look for the host and port the application is trying to bind to.

**Synthesis:** "The environment contains a Flask application in `app.py` and a `requirements.txt` file. The `Flask` package is listed in `requirements.txt` but is not currently installed in the environment (output of `pip list` was empty). There are no other Python processes running. The first step should be to install the dependencies with `pip install -r requirements.txt`."
