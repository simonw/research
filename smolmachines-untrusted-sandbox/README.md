# smolmachines / smolvm as a sandbox for untrusted Python & JavaScript

**Investigation date:** 2026-08-19 · **smolvm version tested:** 1.8.3
([smolmachines.com](https://smolmachines.com), [smol-machines/smolvm](https://github.com/smol-machines/smolvm))

## TL;DR

smolvm is a good fit for the "run user-provided data transformations" use case.
Everything needed is a first-class CLI flag — no wrapper hacks required:

```bash
smolvm machine run \
  --image ./python.tar \        # local docker-save tar → no network ever needed
  --cpus 1 --mem 512 \          # CPU + RAM caps
  --timeout 30s \               # kills "while True" (enforced by guest agent)
  --storage 3 \                 # caps ALL guest disk writes at 3 GiB
  --unprivileged \              # defense-in-depth caps drop inside the guest
  -v "$PWD/in:/in:ro" \         # designated input, read-only
  -v "$PWD/out:/out" \          # designated output, read-write
  -- python3 /in/transform.py
```

The isolation boundary is a hardware VM (libkrun on KVM / Hypervisor.framework /
WHP) with its own guest kernel — not a shared-kernel container. With no `--net`
the VM gets **no network device at all** (verified in source:
`plan_launch_network` returns backend `None`).

See `notes.md` for the full investigation log, `run-tests.sh` /
`run-tests-round2.sh` for the test batteries (with results in
`results-round1.log` / `results-round2.log`), and `sandbox-run.sh` for a
ready-to-use wrapper.

## Test results (GitHub Actions ubuntu runner, KVM)

This container has no KVM (it's itself a Firecracker guest without nested
virt), so the battery ran on a GitHub Actions `ubuntu-latest` runner, which
exposes `/dev/kvm` ([run 1](https://github.com/simonw/research/actions/runs/32312341067),
[run 2](https://github.com/simonw/research/actions/runs/32312932052)).
14 tests in round 1, 12 passed as designed; the 2 "failures" were both real
findings, and round 2 confirmed the fixes for them (see below).

| Test | Result |
|---|---|
| T1 cold boot (`machine run`, local alpine tar, no net) | **577–643 ms** per full create-boot-exec-teardown cycle |
| T2/T3 Python & Node hello (offline images) | both work, stdout/exit codes propagate |
| T4 network truly off | `wget` and DNS both fail with no `--net` |
| T5 `while True:` spin, `--timeout 10s` | killed at 11 s wall, exit 124, **zero leftover VMM processes** |
| T6 1 GiB alloc with `--mem 256` | MemoryError inside guest, exit 1; host memory unaffected |
| T7 fork bomb, `--cpus 1` | VM exits in ~1 s (guest PID space exhausts), host load 0.69, clean teardown |
| T8 disk bomb with `--overlay 1` | **FINDING: `--overlay` does not bound `/` writes** — guest wrote 4 GB to the 20 GiB storage disk. Use `--storage N` (round 2: `--storage 3` → ENOSPC as expected) |
| T9/T10 ro/rw volumes + CSV→JSON (py), JSON→TSV (node) | ro enforced, only mounted dirs shared (virtiofs), transforms round-trip |
| T11 persistent machine | start 1.5 s, then **warm `exec` 48 ms**; `exec --timeout 5s` kills spin, machine stays healthy |
| T12 HTTP API | exec + file upload/download work. **FINDING: field is `timeoutSecs`** (camelCase); a `timeout_secs` field is silently ignored and the spin ran to a 300 s connection timeout. With correct casing (round 2): killed at 5 s, machine healthy |
| T13 `--unprivileged` | CapEff drops from `1ffffffffff` (full) to `a80425fb` (standard unprivileged set) |
| T4b registry image with no `--net` | workload never executes: the in-guest pull itself gets "network is unreachable" (round-2 R4) — the no-net VM has no network path at all. `machine create` refuses this combination up front; `machine run` fails at pull time with a helpful hint |
| R3 (round 2) mount inventory | only the two requested virtiofs shares cross the VM boundary (`/in` ro, `/out` rw); `/`, `/workspace`, `/tmp` are all guest-local |

## What each requirement maps to

| Requirement | smolvm mechanism | Verified |
|---|---|---|
| RAM cap | `--mem <MiB>` — VM-level allocation, elastic via virtio-balloon | T6: 1 GiB alloc in a 256 MiB VM fails; host unaffected |
| CPU cap | `--cpus <N>` — vCPU count limits host CPU consumption | T5/T7 |
| "while true" protection | `--timeout <dur>` on `run` and `exec`; `timeoutSecs` in the HTTP API | T5: spin killed on schedule, VM torn down, no leftover processes |
| No network | Default. No device attached without `--net` | T4: wget + DNS both fail |
| Designated files only | `-v HOST:GUEST[:ro]` (directory granularity) or `machine cp` (file granularity); guest sees nothing else of the host | T9: ro enforced, rw round-trips |
| Disk-fill protection | `--storage <GiB>` — the guest rootfs overlay lives on this disk (`--overlay` does NOT bound `/` writes) | T8 + round-2 R1: `--storage 3` → guest fs 100% full at 2.9 GB, dd stops |
| Fork bombs | Guest kernel owns the PID space; `--cpus`+`--timeout` bound the damage | T7 |

## Things to know before building on it

1. **Image pulls happen inside the guest**, so `--image python:3.12-alpine`
   with no `--net` cannot work — `machine create` refuses the combination up
   front, and `machine run` fails at pull time with "network is unreachable"
   (the no-net VM genuinely has no route out). For a fully offline sandbox, feed it
   local images: `docker save python:3.12-alpine -o python.tar` once, then
   `--image ./python.tar` forever. Alternatives: pack a `.smolmachine` artifact
   (`pack create`), or use `--allow-host`/`--allow-cidr` egress filtering
   (first boot auto-allows just the registry for the pull).
2. **Per-task cold start vs warm pool.** `machine run` is fully ephemeral
   (best isolation, ~1–2 s per task including image rehydration). For high
   throughput, create a persistent no-net machine and `machine exec` per task
   (~50–150 ms), accepting that filesystem state persists between execs — or
   use `machine fork` (CoW clone of a running "golden" VM, `--hold` for
   pre-booted pool slots) to get both speed and a fresh state per task. The
   source notes warm pools of network-less machines built from packs are the
   intended production pattern.
3. **The HTTP API (`smolvm serve`) is unauthenticated on localhost.** Bind it
   to a unix socket (`--listen $XDG_RUNTIME_DIR/smolvm.sock`) and control
   access with file permissions. (Fleet mode has mTLS and deliberately
   restricts the loopback door to health endpoints — the codebase is
   security-conscious about this.)
4. **`--timeout` is enforced by the guest agent** (kills the process, returns
   captured output) with a host-side client timeout as backstop. For belt and
   braces wrap the CLI call in coreutils `timeout` too — a wedged VM then still
   gets torn down (ephemeral `run` VMs are discarded on exit either way).
5. **Volumes are directories, not files.** "Designated files" means staging
   them into an input dir (mounted `:ro`) and collecting from an output dir.
   `machine cp` gives per-file transfer for persistent machines (4 GiB cap per
   transfer).
6. **Host requirements:** bare-metal Linux with `/dev/kvm`, macOS 11+, or
   Windows with WHP. It does NOT run inside most containers/VMs without nested
   virt — this investigation's own container (a Firecracker guest) couldn't
   run it, which is why the battery runs on a GitHub Actions runner (which
   does expose KVM).
7. **Installer bug found:** behind a proxy that intercepts `api.github.com`,
   `install.sh` mis-parses the "latest" version and 404s. Pin the version:
   `curl -sSL https://smolmachines.com/install.sh | bash -s -- --version 1.8.3`.

## Recommended architecture for a data-transformation service

```
                 ┌──────────────────────── host ───────────────────────┐
 user code ──►   │ orchestrator ──► smolvm machine run                 │
 user data ──►   │   stage task dir:   /tasks/<id>/in  (code + data)   │
                 │   collect results:  /tasks/<id>/out                 │
                 └──────────────────────────────────────────────────────┘
   per task: --cpus 1 --mem 512 --storage 3 --timeout 30s --unprivileged
             -v /tasks/<id>/in:/in:ro  -v /tasks/<id>/out:/out   (no --net)
```

- One `machine run` per task = zero cross-task contamination; everything the
  task can touch is `/in` (ro), `/out` (rw), and a 3 GiB throwaway storage disk.
- Exit code, stdout, stderr propagate through the CLI; timeouts surface as a
  non-zero exit.
- Scale-up path: `smolvm serve` on a unix socket + persistent machines or fork
  pools when per-task VM boot becomes the bottleneck.

`sandbox-run.sh` in this folder implements the per-task wrapper.
