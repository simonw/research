# Strategy Generation Skill

**Objective:** To move beyond a single, reactive solution and instead develop a portfolio of well-reasoned strategies. This skill forces a deeper consideration of the problem and its potential pitfalls.

**Instructions:**

When you invoke this skill, you must generate a comprehensive strategy document that includes the following sections:

**1. Knowledge Extraction:**
*   Based on the information you have gathered so far, what are the key concepts, technologies, and potential challenges related to this task?
*   What is your initial hypothesis about the root cause of the problem?

**2. Alternative Approaches:**
*   Describe at least two distinct, viable strategies for solving the problem.
    *   **Strategy A:** The most direct and likely approach.
    *   **Strategy B:** A plausible alternative if Strategy A fails.
*   For each strategy, provide a high-level sequence of commands or code modifications.

**3. Risk Assessment:**
*   For each proposed strategy, identify any potential risks or high-consequence operations (e.g., deleting files, modifying permissions, making network requests).
*   What are the potential negative consequences of each strategy?

**4. Common Failures and Remediation:**
*   What are the most common ways that a task of this type can fail? (e.g., syntax errors, dependency conflicts, permission errors, network timeouts).
*   For each potential failure, suggest a specific remediation step.

**Example Usage:**

**Task:** "I need to deploy this Django application to a new server."

**Strategy Document:**

*   **Knowledge Extraction:**
    *   **Concepts:** Django, Gunicorn, Nginx, systemd, PostgreSQL.
    *   **Hypothesis:** The task requires a standard Django deployment stack. The main challenge will be configuring Gunicorn and Nginx to work together correctly.

*   **Alternative Approaches:**
    *   **Strategy A (Gunicorn + Nginx):**
        1.  Install dependencies from `requirements.txt`.
        2.  Configure Gunicorn to serve the Django application.
        3.  Create a systemd service file to manage the Gunicorn process.
        4.  Configure Nginx as a reverse proxy to Gunicorn.
    *   **Strategy B (Dockerized Deployment):**
        1.  Create a `Dockerfile` for the Django application.
        2.  Create a `docker-compose.yml` file to manage the Django, Nginx, and PostgreSQL services.
        3.  Build and run the Docker containers.

*   **Risk Assessment:**
    *   **Strategy A:** Incorrectly configuring Nginx could expose the application to security vulnerabilities. A misconfigured systemd service could fail to restart the application after a crash.
    *   **Strategy B:** A poorly written `Dockerfile` could result in a large, insecure image. Docker networking can be complex to debug.

*   **Common Failures and Remediation:**
    *   **Failure:** Nginx returns a 502 Bad Gateway error.
        *   **Remediation:** Check the Gunicorn socket or port to ensure it's running and accessible to Nginx. Verify the `proxy_pass` directive in the Nginx configuration.
    *   **Failure:** Django `collectstatic` command fails.
        *   **Remediation:** Ensure that the `STATIC_ROOT` setting in `settings.py` is correctly configured and that the target directory is writable.
