# Notes: smolmachines / smolvm as an untrusted-code sandbox

Goal: evaluate https://smolmachines.com (smolvm) for running untrusted Python/JS with:
- RAM limits and CPU-time limits (protection against `while true`)
- No network access
- Filesystem access restricted to designated files
- Use case: executing user-provided data-transformation tasks

## Initial research (web)

- smolmachines.com = product site for `smolvm` (GitHub: smol-machines/smolvm, ~4.6k stars)
- Portable, hardware-isolated Linux VMs; VMM is libkrun with custom kernel (libkrunfw)
- Backends: KVM (Linux), Hypervisor.framework (macOS), WHP (Windows)
- Claims: boots <200ms, network disabled by default, host allowlists, `.smolmachine`
  portable artifacts, default 4 vCPU / 8GB RAM elastic
- Install: `curl -sSL https://smolmachines.com/install.sh | bash`
- Basic run: `smolvm machine run --image alpine -- sh -c "..."`

## Environment check

- This Claude Code container: Linux 6.18.5-fc-v20 (itself a Firecracker guest),
  4 vCPU, 15GB RAM. **No /dev/kvm, no vmx/svm CPU flags** → no nested virt.
- `smolvm machine run` fails as expected: "kvm not available".
- Plan B: GitHub Actions ubuntu runners DO expose /dev/kvm → run the real test
  battery via a temporary workflow on this branch, collect logs, remove
  workflow in final commit.

## Install experience

- `install.sh` from smolmachines.com: detects platform, warns clearly about
  missing KVM, verifies sha256 checksums. BUG: in this proxied environment the
  installer's `api.github.com/releases/latest` call returned a proxy error JSON
  and the script mis-parsed version "0.1.1" → 404. Workaround:
  `bash install.sh --version 1.8.3` (real latest as of 2026-08-19).
- Installs to ~/.smolvm, symlink in ~/.local/bin. No daemon.

## Source review (cloned smol-machines/smolvm @ e432c7e, v1.8.3, Rust)

Everything needed for the untrusted-code use case exists as first-class flags:

| Need | Mechanism |
|------|-----------|
| RAM cap | `--mem <MiB>` (default 8192); elastic via virtio-balloon |
| CPU cap | `--cpus <N>` (default 4) |
| "while true" protection | `--timeout <DURATION>` on `machine run` AND `machine exec`; HTTP API exec takes `timeout_secs` (src/api/handlers/exec.rs:133) |
| No network | off by default; `--net` is opt-in; egress filters `--allow-host`/`--allow-cidr`; `--outbound-localhost-only` |
| Designated files only | `-v HOST:GUEST[:ro]` (directories only); `machine cp` for file-level; guest sees nothing else of host FS |
| Disk-fill protection | `--overlay <GiB>` (default 2) + `--storage <GiB>` (default 20) |
| Defense in depth | `--unprivileged` — restricted caps, read-only cgroup inside guest |

Key subtleties found in source:

1. **Image pulls run INSIDE the guest.** A registry ref (`--image alpine`)
   with no `--net` is refused up front (`validate_image_fetchable`,
   src/config.rs:834; comment at src/cli/vm_common.rs:691). No-net options:
   - `--image ./img.tar` (docker/podman save archive) or `./rootfs/` dir
   - `.smolmachine` packs (`pack create` once, then `--from pack.smolmachine`)
   - `--allow-host`/`--allow-cidr`: first boot auto-allows the registry host
     for the pull (`allow_image_pull_egress`, src/agent/launcher.rs:373)
2. **Warm-pool pattern is intentional**: src/config.rs comment — "pooled VMs
   are deliberately created network-less from a pack".
3. **`machine fork`**: CoW clone (memfd RAM + disks) of a running "golden"
   VM; `--hold` parks pre-booted pool slots; fork pools with leases exist in
   `serve` (src/pool.rs). Aimed at CUDA but works for generic sandbox pools.
4. Secrets: refs resolved host-side; HTTP API bodies may NOT carry secret refs
   (arbitrary-host-file-read prevention) — good security posture.
5. `/workspace` persists per-machine; `/tmp` is tmpfs.

## Test battery via GitHub Actions

Wrote `run-tests.sh` (committed in this folder) + temporary workflow
`.github/workflows/smolvm-sandbox-test.yml`. Runner recipe for KVM:
udev rule to chmod /dev/kvm 0666 (standard Android-emulator-on-Actions trick).

Images delivered offline via `docker save` tars so every sandbox run is
`--net`-free end to end.

## More source findings (while CI ran)

- **Timeout enforcement**: guest agent kills the child on deadline
  (`wait_with_timeout_cleanup_and_liveness`, crates/smolvm-agent/src/process.rs)
  and also kills it if the client disconnects; host client sets its own read
  timeout guard (src/agent/client.rs `set_exec_timeout`). Output captured up to
  the kill is returned.
- **No-net = no device**: `plan_launch_network` (src/network/launch.rs:89)
  returns backend `None` unless --net/ports/egress policy/fabric requested.
  Comment notes TSI's in-libkrun egress filter is NOT trusted — egress policies
  force the virtio-net backend where the host-side stack enforces the
  allow-list. Fleet mode adds a hard floor denying metadata/internal/loopback.
- **Volumes**: virtiofs (krun_add_virtiofs), root uses a 512MB DAX window.
- **serve API**: unauthenticated on the plain listener — bind to a unix socket
  for local services. Fleet mode: mTLS port has full API; loopback door
  restricted to health/capacity/metrics specifically to close an SSRF pivot
  (attacker-supplied registry ref pointing at 127.0.0.1). Tests enforce this.
- **File upload cap via API**: 100 MiB per request (MAX_FILE_UPLOAD_BYTES);
  `machine cp` caps at 4 GiB per transfer.
- **Fork pools**: examples/headless-browser docs: golden VM cold-starts once,
  CoW clones materialize in ~50–130ms with already-warm processes. Fork pool
  records + leases in src/pool.rs; serve exposes /pools/{name}/leases.
- **Cloud**: smolmachines.com/pricing — hosted microVMs w/ REST API, free tier
  $10/mo credit, warm-pool starts. Same .smolmachine artifacts. Not tested
  (needs account); local OSS tool was the focus.

## CI results — round 1 (run 32312341067, ubuntu-latest, KVM OK)

PASS=12 FAIL=2 of 14.

- T1 cold boot from local alpine tar: 643/580/577/591/588 ms end-to-end.
- T5 spin + --timeout 10s: rc=124 at 11s, no leftover processes. 
- T6 --mem 256 vs 1GiB alloc: rc=1, host MemAvailable barely moved.
- T7 fork bomb: returned in 1s rc=2 (guest sh dies), host load 0.69.
- T11: create+start 1524ms; warm execs 162/79/48/48/48 ms; exec --timeout
  killed spin at 5s and machine remained usable.
- T13: CapEff 000001ffffffffff (default, VM-grade full caps) vs
  00000000a80425fb (--unprivileged, standard container cap set).

FAILURES (both informative):
- T8: --overlay 1 did NOT bound writes to `/` — dd wrote 4GB fine; guest /
  showed a 19.6G fs (the 20GiB storage disk). Host / lost ~3GB during the
  test (returned after ephemeral cleanup). => disk cap needs --storage.
- T12: HTTP API exec ran the spin for 300s ignoring my timeout field. Root
  cause: ExecRequest is #[serde(rename_all = "camelCase")] (src/api/types.rs:134)
  → field is `timeoutSecs`; unknown `timeout_secs` silently ignored (serde
  default). My bug, but also an API footgun: no unknown-field rejection.
- T9 footnote: my "HOST-FILES-VISIBLE" heuristic (ls /root /home non-empty)
  just saw the guest's own dirs — bogus signal, mount isolation itself held.
- T4b surprise: `machine run` with registry image + no --net did NOT refuse
  up front (that's the `create` path); it pulled the image (pull egress is
  auto-allowed just for the pull, per allow_image_pull_egress) and still
  failed rc=1 before running the workload. Round 2 captures the full error.

## Round 2 (run 32312932052) — PASS=3 FAIL=1 (the FAIL was my assertion)

- R1 (--storage 3 disk bomb): dd stopped at 2.9GB, guest / 100% full → the
  cap works. My grep asserted on "no space" but the dd error line fell
  outside the `tail -2` capture and rc=0 came from the trailing df. Verdict:
  use --storage for disk caps, not --overlay.
- R2 (API timeoutSecs): {"exitCode":124,"stderr":"command timed out after
  5000ms"} at exactly 5s; machine healthy after. Round 1's T12 failure was
  purely the snake_case field being silently ignored.
- R3 (mount inventory): only smolvm0:/in (ro,sync) and smolvm1:/out (rw,sync)
  virtiofs shares cross the boundary. Guest / is overlayfs with upper on
  /storage (= /dev/vda, the --storage disk) — which is why --storage is the
  write bound. /workspace also on /dev/vda. /tmp tmpfs.
- R4 (registry + no net, machine run): pull fails in-guest with "dial udp
  1.1.1.1:53: network is unreachable" + friendly hint to add --net. So round
  1's "Pulling... done." line was cosmetic; no data was fetched. The no-net
  guarantee covers the pull path too. (`create` refuses up front instead.)

## Wrap-up

- sandbox-run.sh updated to use --storage 3 (not --overlay).
- Temporary workflow .github/workflows/smolvm-sandbox-test.yml removed in the
  final commit; both CI runs remain viewable under the repo's Actions tab.
- Cleaned CI logs saved as results-round1.log / results-round2.log.
