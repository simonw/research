Durable execution workflows can be implemented using SQLite, as demonstrated by the Absurd-in-SQLite project, which is inspired by Armin Ronacher's Absurd. This project provides a proof-of-concept implementation of durable execution using SQLite, allowing for reliable and long-running workflows that can survive crashes and network failures. The project utilizes a pull-based model, where workers pull tasks from a queue, and features a replay model that replays the entire function from the beginning when a task resumes. For more information, visit the [Absurd](https://github.com/earendil-works/absurd) and [Absurd Workflows](https://lucumr.pocoo.org/2025/11/3/absurd-workflows/) resources.

* Key findings:
  * Durable execution can be achieved using a database like SQLite
  * The replay model allows for reliable execution of tasks
  * A pull-based model can simplify workflow management
  * SQLite can handle moderate workloads, but Postgres may be more suitable for high-concurrency scenarios
