# Advanced Web Search Skill

**Objective:** To conduct a thorough and effective web search to gather the information needed to solve a task. This skill is designed to find precise, actionable solutions rather than generic tutorials.

**Instructions:**

When you invoke this skill, you must follow this multi-round research protocol. You may conduct up to three rounds of searching.

**Round 1: Initial Broad Search & AI Overview**

1.  **Query Generation:** Formulate a concise, high-level search query that captures the core of the problem.
2.  **Execute Search:** Perform a Google search with your query.
3.  **Extract Google AI Overview:** **Immediately prioritize and extract the full text of the Google AI Overview summary.** This often contains a highly condensed and accurate solution.
4.  **Initial Analysis:** Briefly review the top 3-5 search results to get a general sense of the problem space.

**Round 2: Deep Dive with High-Specificity Queries**

1.  **Refined Query Generation:** Based on your initial findings, create one or more *highly specific, low-frequency* search queries. Include specific error messages, library names, and function names.
    *   **Good Example:** `"flask sqlalchemy 'InterfaceError' (sqlite3.InterfaceError) invalid command"`
    *   **Bad Example:** `flask database error`
2.  **Source Prioritization:** Add `site:github.com` or `site:stackoverflow.com` to your queries to prioritize results from these platforms, as they are more likely to contain actionable code.
3.  **Deep Link Exploration:** Open the top 3 search results (excluding any you've already reviewed) and analyze their content in detail. Look for code snippets, configuration examples, and explanations of the underlying problem.

**Round 3: Verification and Alternative Solutions**

1.  **Query for Alternatives:** If the previous rounds did not yield a definitive solution, search for alternative approaches or common pitfalls.
    *   **Example Query:** `"python file upload alternative to flask-uploads"`
2.  **Quality Control:** Throughout the process, actively filter out any search results that mention "Terminal Bench" or other benchmark-related terms to avoid contaminating your solution.
3.  **Synthesize Findings:** After completing your search rounds, create a consolidated summary of your findings. This summary should include:
    *   The most promising solution(s).
    *   Key code snippets or commands.
    *   A list of the URLs you found most helpful.

**Example Usage:**

**Task:** "I'm trying to connect to a PostgreSQL database with Python, but I'm getting a `psycopg2.OperationalError: could not connect to server` error."

**Search Process:**

*   **Round 1 Query:** `python psycopg2.OperationalError: could not connect to server`
    *   **Action:** Extract the Google AI Overview, which likely suggests checking the database connection string, firewall rules, and whether the database server is running.
*   **Round 2 Query:** `"psycopg2.OperationalError" connection refused site:stackoverflow.com`
    *   **Action:** Analyze the top StackOverflow answers, looking for code examples of correct connection strings and `pg_hba.conf` configurations.
*   **Round 3 Query:** `python postgresql connection best practices`
    *   **Action:** Look for articles on connection pooling or using context managers to ensure connections are properly closed.
*   **Synthesis:** "The `psycopg2.OperationalError` is likely due to an incorrect connection string or a firewall issue. The most common solution is to ensure the host, port, user, and password are correct in the DSN. This [StackOverflow link] provides a canonical example of a correct connection string. Additionally, one should check that the PostgreSQL server is running and that port 5432 is open on the firewall."
