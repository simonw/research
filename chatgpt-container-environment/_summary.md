Experiments in the ChatGPT sandbox reveal that general outbound internet access from Python and other user code (such as HTTP requests) is entirely blocked, while package managers like pip and npm are permitted to fetch dependencies using curated internal registry proxies. The container provides a privileged fetching mechanism (`container.download`) for select public URLs, which is more powerful than standard code-based networking. Metadata inspection shows that packages installed through these proxies behave normally and are introspectable via Python standards. While Docker CLI tools are absent, the internal Artifactory proxy allows programmatic access to Docker registry endpoints, highlighting a clear pattern: only curated package egress is supported, not arbitrary web access. Further documentation of internal registry endpoints illustrates broad, multi-language support for curated package downloads, but not unmediated internet access.

**Key findings:**
- Public URLs are retrievable with [container.download](https://platform.openai.com/docs/guides/code/container-download), not with standard code requests.
- All package managers (pip, npm, uvx) rely on internal registry mirrors ([Artifactory endpoints](https://jfrog.com/artifactory/)).
- Arbitrary repository clones (GitHub via git/curl) fail; PyPI source artifacts work instead.
- Docker manifests and blobs can be fetched via registry endpoints, though no Docker runtime exists.
- Registries for Go, Maven, Gradle, Cargo are configured but were not fully tested.
- Practical access to external data is constrained to what registries and container.download allow, enforcing a strong sandbox model.
