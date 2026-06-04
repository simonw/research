# What we learned today about this ChatGPT “container” environment

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

_Date: 2026-01-26 (America/Los_Angeles)_

These notes summarize what we observed by actually running commands and code inside the ChatGPT container/sandbox environment.

---

## 1) `container.download` can fetch public URLs, but Python code cannot

### What we did
- We used **`container.download`** to fetch:

  - `https://simonwillison.net/for-chatgpt-container`

- We then counted **lines** and **characters** in the downloaded file using Python.

### What we observed
- The file downloaded successfully via `container.download`.
- Counting in Python showed:
  - **Lines:** 3
  - **Characters:** 28 (as-saved, including `\r\n` line endings)
  - **Characters after normalizing `\r\n` → `\n`:** 26

### Key takeaway
- **`container.download` has network reachability that ordinary Python networking does not.**
- It behaves like a privileged fetch mechanism, separate from user-code egress.

---

## 2) `pip install` works because it uses an internal package proxy (not direct internet)

### The “contradiction”
- You observed that:
  - `urllib` could not fetch a normal web URL (blocked)
  - but `pip install` worked (seemingly needs the internet)

### What explains it
- `pip` is configured to talk to an **internal package gateway / mirror** rather than the public internet.
- In this environment, general outbound network from arbitrary code is blocked, but **package managers are allowed** to use an internal registry endpoint.

### Evidence we collected
- Relevant environment variables (password redacted):
  - `PIP_INDEX_URL=https://reader:****@packages.applied-caas-gateway1.internal.api.openai.org/.../pypi-public/simple`
  - `PIP_TRUSTED_HOST=packages.applied-caas-gateway1.internal.api.openai.org`

- `pip config debug -v` showed those settings coming from the environment.

- Running verbose pip operations showed downloads coming from the internal host, not from `pypi.org`.

### Key takeaway
- **Python code cannot reach the open internet**, but `pip` can fetch from a **whitelisted internal proxy**.

---

## 3) We installed packages and inspected their metadata in Python

### Packages
From `https://simonwillison.net/for-chatgpt-container`, we identified and installed:
- `datasette`
- `sqlite-utils`
- `llm`

### What we inspected
Using `importlib.metadata`, we extracted distribution metadata (examples: version, summary, requires-python, entry points).

### Key takeaway
- Package installation is fully functional through the internal PyPI proxy.
- Installed distribution metadata is available via standard Python introspection.

---

## 4) Trying to fetch the URL directly with `urllib` fails

### What we did
- Attempted `urllib.request.urlopen("https://simonwillison.net/for-chatgpt-container")`.

### What we observed
- It failed immediately with a network error:
  - `URLError: <urlopen error [Errno 101] Network is unreachable>`

### Key takeaway
- **General outbound networking from user code is blocked.**

---

## 5) `uvx`, `npm install`, and `npx` work via the same “internal registry” idea

### `uv` / `uvx`
- `uvx` works because it is configured with an internal Python package index too.
- Relevant environment variables (password redacted):
  - `UV_INDEX_URL=https://reader:****@packages.applied-caas-gateway1.internal.api.openai.org/.../pypi-public/simple`
  - `UV_INSECURE_HOST=https://packages.applied-caas-gateway1.internal.api.openai.org`

### Node tooling (`npm` / `npx`)
- Node tools can fetch packages because npm is configured to use an internal registry endpoint.
- Environment variable (password redacted):
  - `NPM_CONFIG_REGISTRY=https://reader:****@packages.applied-caas-gateway1.internal.api.openai.org/.../npm-public`

### Key takeaway
- Multiple ecosystems have curated egress:
  - **Python (`pip`, `uv/uvx`)**
  - **Node (`npm`, `npx`)**
- The pattern is consistent: **package managers use internal mirrors**.

---

## 6) Anonymous GitHub clone / repo ZIP fetch does NOT work from the container

### What we tried (against `simonw/datasette`)
- `git clone --depth 1 https://github.com/simonw/datasette.git ...`
- `curl -L https://github.com/simonw/datasette/archive/refs/heads/main.zip ...`

### What we observed
- Both attempts failed due to inability to reach GitHub over the network.

### `container.download` and GitHub ZIPs
- `container.download` can fetch some URLs, but GitHub archive downloads often redirect to `codeload.github.com`.
- That redirect path is not usable in this environment (the fetch/open safety restrictions block it).

### Workaround we noted
- If you just need release source (not the Git history/main branch), use PyPI artifacts:
  - `pip download --no-binary :all: datasette -d ...`
  - This fetches the source distribution tarball via the internal PyPI proxy.

### Key takeaway
- **GitHub is not generally reachable** (clone/zip).
- **PyPI is reachable** (via internal proxy), which can provide released source.

---

## 7) Other interesting environment variables for registries across languages

We found a set of “CAAS Artifactory” variables that advertise internal registries for multiple ecosystems:

- `CAAS_ARTIFACTORY_BASE_URL=packages.applied-caas-gateway1.internal.api.openai.org`

Per-ecosystem endpoints (paths abbreviated):
- PyPI: `CAAS_ARTIFACTORY_PYPI_REGISTRY=.../artifactory/api/pypi/pypi-public`
- npm: `CAAS_ARTIFACTORY_NPM_REGISTRY=.../artifactory/api/npm/npm-public`
- Go: `CAAS_ARTIFACTORY_GO_REGISTRY=.../artifactory/api/go/golang-main`
- Maven: `CAAS_ARTIFACTORY_MAVEN_REGISTRY=.../artifactory/maven-public`
- Gradle: `CAAS_ARTIFACTORY_GRADLE_REGISTRY=.../artifactory/gradle-public`
- Cargo: `CAAS_ARTIFACTORY_CARGO_REGISTRY=.../artifactory/api/cargo/cargo-public/index`
- Docker: `CAAS_ARTIFACTORY_DOCKER_REGISTRY=.../dockerhub-public`

Credentials are provided via:
- `CAAS_ARTIFACTORY_READER_USERNAME=reader`
- `CAAS_ARTIFACTORY_READER_PASSWORD=****`

We also saw a policy hint:
- `NETWORK=caas_packages_only`

### Key takeaway
- This environment is explicitly designed for **“packages-only” egress** via internal mirrors.

---

## 8) Docker: no Docker CLI/daemon, but the internal Docker registry proxy is reachable

### What we tried
- `docker pull ...` (and `docker`, `podman`, `buildah`)

### What we observed
- The tooling is **not installed**:
  - `docker: command not found`
  - `podman/buildah: not found`

### What still works
Even without Docker tools, the **registry itself** is reachable and speaks the Docker Registry v2 API.

- `GET https://packages.applied-caas-gateway1.internal.api.openai.org/dockerhub-public/v2/`
  - returns **HTTP 200**
  - includes `Docker-Distribution-Api-Version: registry/2.0`
  - includes JFrog Artifactory headers

We also fetched and inspected real image metadata:
- Example: `library/hello-world:latest`
  - Retrieved an OCI image index
  - Followed the `linux/amd64` manifest digest
  - Downloaded the **config blob** successfully with Python

### Key takeaway
- **Docker images can be accessed as registry artifacts** (manifests/blobs) through the internal proxy,
  even though the Docker runtime/CLI isn’t present.

---

## 9) Overall mental model of this environment

### Two egress “lanes”
1) **General-purpose networking from user code is blocked**
   - Python `urllib`, Node `fetch`, etc. fail with network errors.

2) **Package/registry access is permitted via internal mirrors**
   - Python: `pip`, `uv/uvx` via internal PyPI proxy
   - Node: `npm`, `npx` via internal npm proxy
   - Docker registry endpoints reachable via internal Artifactory proxy
   - Additional ecosystems advertised (Go, Maven/Gradle, Cargo), but not all were tested end-to-end.

### Practical implication
- If you need external content:
  - Prefer **`container.download`** for one-off URL fetches that it allows.
  - Prefer **package registries** (PyPI/npm/etc.) for code and artifacts.
  - Assume arbitrary web access from Python/Node is blocked.

---

## Appendix: what we did not fully test (but is suggested by env vars)
- Go modules via `CAAS_ARTIFACTORY_GO_REGISTRY` (and whether `GOPROXY` is configured to use it)
- Maven/Gradle dependency resolution via the internal registries
- Rust/Cargo via the internal cargo registry (also note: `cargo` may not be installed by default)

If you want, we can run targeted experiments for any of those ecosystems.
