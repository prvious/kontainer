# Stack deployment in Kontainer v2

Status: proposed architecture and compatibility contract  
Research date: 2026-08-19  
Scope: pure-Rust Docker Swarm stack validation, planning, deployment, update, observation, rollback, repair, and removal

## Executive decision

Kontainer v2 should build a Kontainer-owned stack reconciler in Rust. It should accept a deliberately bounded Compose Specification subset, normalize it into a versioned desired-state plan, compare that plan with resources carrying strong Kontainer ownership, and execute the result as a durable, resumable operation through the EngineClient proposed in [swarm-resources.md](./swarm-resources.md).

The runtime must not shell out to Docker CLI, Docker Compose, or a Go helper. Docker CLI 29.7.2 remains the compatibility oracle for the supported overlap, not a production dependency. Kontainer should reproduce the useful conventions operators already expect:

- names scoped as stack_resource;
- the com.docker.stack.namespace label on stack resources;
- com.docker.stack.image preserving the requested service image;
- an overlay default network;
- external resources never created or deleted;
- services updated in place with Engine object versions;
- named volumes retained on stack removal;
- tag-to-digest resolution and optional registry-auth propagation.

Kontainer must intentionally improve the unsafe parts of Docker CLI stack reconciliation: strict preflight before mutation, stable ordering, ownership stronger than a namespace label, optimistic concurrency, idempotency, operation locking, per-step journaling, explicit prune policy, bounded rollout observation, cancellation, stack-level rollback, and crash recovery.

The resulting boundary is:

~~~text
source bundle + explicit variables + credential references
  -> secure Compose loader
  -> normalized StackPlan
  -> policy and capability validation
  -> observed owned-resource snapshot
  -> deterministic StackDiff
  -> durable StackOperation
  -> EngineClient resource mutations
  -> rollout observer
  -> committed StackRevision or actionable partial result
~~~

This subsystem controls desired state. Docker Swarm remains authoritative for actual service scheduling and task state. A successful Engine mutation means that Swarm accepted new desired state; it does not mean the workload converged.

## Goals and non-goals

The design covers:

- validation and preview of uploaded Compose source;
- first deploy and repeat deploy to the same stack;
- create/update/rotate/remove behavior for services, networks, configs, secrets, and service volume mounts;
- registry credentials and image digest resolution;
- diff, prune, rollout progress, cancellation, rollback, repair, and history;
- one-controller SQLite compatibility and multi-controller shared-database recovery;
- integration with v1 service mutations and the current desktop expectations.

The initial stack subsystem does not:

- build or push images;
- clone Git repositories or continuously reconcile a branch;
- implement a general Compose local-container runtime;
- provide Kubernetes or Compose-up semantics;
- delete named volumes by default;
- promise a transaction across Docker Engine resources;
- expose the Docker socket, registry passwords, config bytes, or secret bytes to React.

## Evidence from this repository

### What v1 actually implements

The current Go agent has no deploy endpoint. A stack is only a derived grouping of services whose service-level label com.docker.stack.namespace is non-empty.

The concrete behavior is:

| Area | Current behavior | Consequence for v2 |
|---|---|---|
| Stack list | server/app/services/docker/swarm/stacks.go lists all services, groups the namespace label, lists desired-running tasks, and totals service/running/replicated desired counts. | Preserve a compatibility-derived view, but replace ambiguous replica arithmetic with a convergence model. Current global and job counts are wrong or absent. |
| Stack identity | Name is the namespace label. There is no Engine stack object, database row, desired document, revision, source, or owner. | V2 needs a durable stack UUID plus a human name. Namespace-only stacks are legacy/unmanaged until explicitly adopted. |
| Stack removal | Resources matching the namespace label are removed sequentially: services, networks, secrets, then configs. It fails on the first error, has no progress record, does not wait, and retains volumes. A missing stack currently falls through as success. | Retain volumes, but add preview, ownership checks, journaled steps, retries, observation, partial results, and idempotent missing-resource handling. |
| Scale | Inspect service, reject global mode, replace the full spec with replicas changed, using the inspected Engine version. | Route through the shared preservation-safe service patch path and require client/base revision concurrency. |
| Restart | Inspect, increment TaskTemplate.ForceUpdate, replace the spec. | Treat as non-repeatable without a durable idempotency record. Preserve ForceUpdate during ordinary stack deploys. |
| Rollback | Current Goravel code submits rollback=previous without checking PreviousSpec. The older implementation did check. | Restore the guard and distinguish service rollback from stack-revision rollback. |
| Environment/image | Inspect, replace the entire environment list or image, then replace the full service spec. | Stack-owned service edits must share the stack lock and become visible drift or explicit runtime overrides. |

Relevant history confirms the boundary:

- 931a3b6 introduced service mutations, derived stack listing, and service-only removal.
- 0645b68 expanded removal to networks, secrets, and configs.
- a87b7cf added stack replica/update fields and frontend compatibility repairs.
- 1e15746 added the redesigned stack list with a disabled DEPLOY STACK button.

There are important v1 contract bugs that parity work must not copy:

- Tauri scales with PATCH /api/services/{id}, while the Go route is POST /api/services/{id}/scale.
- Tauri expects remove_stack to return removed, while the server returns status.
- Tauri expects updated env/image values, while the server returns only status.
- The current Go removal action never returns ErrNotFound, although the controller maps it.
- The old rollback guard for a missing previous spec was lost in the Goravel port.

### Current UI and product expectations

The current stack list expects name, service count, running/desired replicas, a health indicator, and an updated or last-deploy time. The detail route lists the stack's services. The redesigned DEPLOY STACK button is intentionally disabled. An older StacksList component also contains a destructive confirmation whose copy says only services will be removed, even though the server can also remove networks, secrets, and configs.

The v2 UI contract therefore needs:

- upload/editor input with an explicit stack name and source files;
- Validate and Preview Diff before Deploy;
- field-level diagnostics with file, line, path, severity, and remediation;
- a diff grouped as create, update, rotate, retain, prune, blocked, and no-change;
- explicit confirmation for prune, replacement, legacy adoption, and removal;
- rollout progress by phase, resource, service, and task;
- cancel, retry/repair, deploy previous revision, and stack rollback actions;
- history showing principal, source digest, start/end times, outcome, warnings, and audit ID;
- removal preview that names retained volumes and external resources;
- convergence states rather than inferring health from running >= desired;
- a visible drift/runtime-override state when a stack-owned service is edited directly.

CPU and memory aggregation shown as placeholders on the current list remains outside this subsystem.

## Primary-source findings

### What Docker CLI 29.7.2 actually does

Docker documents stack deploy as accepting Compose version 3.0 and later, with optional service pruning, registry-auth propagation, and image resolution policies always, changed, and never ([docker stack deploy](https://docs.docker.com/reference/cli/docker/stack/deploy/)). The implementation is more informative than the command summary:

1. It loads all files, interpolates each file, validates against the CLI's versioned schema, converts to legacy internal Compose types, and merges the typed configs. It warns for one hard-coded unsupported list, rejects another hard-coded forbidden list, and validates that every service has a valid image ([stack loader](https://github.com/docker/cli/blob/v29.7.2/cli/command/stack/loader.go), [Compose loader](https://github.com/docker/cli/blob/v29.7.2/cli/compose/loader/loader.go), [field policy](https://github.com/docker/cli/blob/v29.7.2/cli/compose/types/types.go)).
2. It verifies that the target daemon is a Swarm manager.
3. If prune is enabled, it deletes old namespace-labelled services before validating external networks or converting and creating the remaining resources.
4. It converts and creates missing networks, defaulting the driver to overlay. Existing stack-labelled networks with the same name are accepted without comparing their configuration.
5. It creates or attempts to update secrets and configs by name.
6. It converts Compose services into full Swarm ServiceSpec values. Names become stack_service; service and container labels receive the namespace; requested images are saved in com.docker.stack.image; maps and selected lists are sorted for idempotence ([Compose-to-Swarm conversion](https://github.com/docker/cli/blob/v29.7.2/cli/compose/convert/service.go), [resource naming and conversion](https://github.com/docker/cli/blob/v29.7.2/cli/compose/convert/compose.go)).
7. It lists existing namespace-labelled services by name and sequentially creates or replaces them. Updates use the current Engine version and preserve ForceUpdate. Image resolution may preserve the current resolved digest when the requested image string did not change ([deployment reconciliation](https://github.com/docker/cli/blob/v29.7.2/cli/command/stack/deploy_composefile.go)).
8. With detach disabled, it observes each service. The observer handles replicated, global, and job modes, detects paused update/rollback states, and requires non-job tasks to remain converged for the update monitor interval ([service progress observer](https://github.com/docker/cli/blob/v29.7.2/cli/command/service/progress/progress.go)).

There is no transaction, operation journal, cross-resource rollback, lock, approval token, or crash resume. Service map iteration is not a dependency schedule. depends_on is loaded but not used by the converter. A failure after earlier mutations leaves them applied. Prune can remove an old service before a later external-network or conversion error. Networks are not reconciled, only created when absent. Deploy prune removes only orphan services, not old networks/configs/secrets. Top-level ordinary volumes are not created as Swarm resources; their driver options are embedded in service mount specifications.

Docker stack remove first lists services, networks, secrets, and configs by namespace, then attempts all four categories in that order. It retains volumes and can optionally wait for tasks to terminate ([Docker CLI removal](https://github.com/docker/cli/blob/v29.7.2/cli/command/stack/remove.go), [docker stack rm](https://docs.docker.com/reference/cli/docker/stack/rm/)).

Kontainer should match observable results for the supported overlap, not reproduce these failure semantics.

### Compose Specification implications

The Compose Specification explicitly allows implementations to support only part of the model and describes default warning, strict rejection, and loose ignore modes for unsupported attributes ([Compose Specification](https://github.com/compose-spec/compose-spec/blob/master/spec.md)). Kontainer should use strict mode by default because ignored deployment fields can create a dangerously different workload.

Important loader rules include ([Compose interpolation](https://docs.docker.com/reference/compose-file/interpolation/), [Compose merge](https://docs.docker.com/reference/compose-file/merge/)):

- top-level version is obsolete and informational;
- interpolation is Bash-like and applied to YAML values before per-file merge;
- multiple files merge mappings, append ordinary sequences, replace command/entrypoint/healthcheck test, and merge ports, volumes, secrets, and configs by defined unique keys;
- relative paths are based on the first Compose file;
- YAML anchors/fragments are document-local;
- top-level name and project-name selection affect generated names;
- deploy is optional in the general specification but contains the Swarm-relevant mode, replicas, resources, placement, restart, update, and rollback policies ([Compose Deploy Specification](https://docs.docker.com/reference/compose-file/deploy/)).

Docker's current merge rules also include !reset and !override tags ([Compose merge rules](https://docs.docker.com/reference/compose-file/merge/)); Docker stack config describes its older merge/interpolation path ([docker stack config](https://docs.docker.com/reference/cli/docker/stack/config/)). Kontainer must version its loader behavior rather than claiming generic latest-Compose compatibility.

### Rust Compose-model options

| Option | Decision | Findings |
|---|---|---|
| compose_spec 0.3.0 | Provisionally accepted as a typed syntax/model aid, behind a Kontainer loader adapter and only after a compatibility/license spike. | It provides idiomatic validated types, short/long syntax handling, extensions, references, and resource validation ([crate docs](https://docs.rs/compose_spec/0.3.0/compose_spec/)). It is not a complete Compose loader: its Options currently handles YAML merge keys only, while interpolation and multi-file merging remain open work ([Options source](https://github.com/k9withabone/compose_spec_rs/blob/7ff87d7b50a568c0e9936a7e7887372b432180fd/src/options.rs), [interpolation issue](https://github.com/k9withabone/compose_spec_rs/issues/37), [merge issue](https://github.com/k9withabone/compose_spec_rs/issues/4)). It uses the archived serde_yaml line and is MPL-2.0, both requiring review. |
| docker-compose-types 0.24.0 | Rejected as the primary model; useful only for fixture comparison. | It is actively published and covers many v2/v3 shapes, but it is a relatively permissive serialization model, not an interpolation/merge/normalization/semantic-validation engine. Its Deploy model also omits parts needed for current Swarm conversion ([crate source](https://github.com/stephanbuys/docker-compose-types/blob/4d3051dcfbe8bd18f4212d631e1578468c0f4db9/src/lib.rs)). |
| Raw Serde structs only | Rejected. | Parsing YAML into structs does not implement Compose interpolation, merge, short-syntax expansion, reference validation, defaults, path rules, diagnostics, or Swarm conversion. |
| Generate types directly from compose-spec.json | Deferred. | It offers schema coverage but generated union types are awkward and schema validation still does not implement loader semantics. It may become a secondary raw-schema validator. |
| Kontainer-owned normalized model | Accepted. | External crate types stop at the loader boundary. StackPlan and Engine conversion remain stable, testable Kontainer domain types. |

The YAML parser itself needs a release decision. The original serde_yaml repository was archived and its final release says no replacement was designated ([serde_yaml release](https://github.com/dtolnay/serde-yaml/releases/tag/0.9.34)). Maintained forks and newer pure-Rust parsers exist, but Compose anchors, merge tags, duplicate-key rejection, spans, YAML 1.1/1.2 scalar behavior, alias limits, and denial-of-service controls must be tested before selection. Do not let compose_spec's current parser dependency silently decide this security boundary.

## Alternatives considered

| Route | Disposition | Reason |
|---|---|---|
| Shell out to docker stack deploy | Rejected for production. | Requires a version-matched binary and config directory, inherits ambient environment/credentials, emits human-oriented progress/errors, provides no resource-level journal or fencing, and makes cancellation and crash recovery opaque. |
| Shell out to docker compose config, then reconcile | Rejected for production; allowed as a test oracle. | It supplies modern compose-go loading but still creates a binary/version dependency and does not perform Compose-to-Swarm conversion. Canonical output can also hide source locations needed for diagnostics. |
| Call compose-go through cgo, FFI, a helper process, or sidecar | Rejected. | Violates the pure-Rust runtime goal and introduces two runtimes, ABI/protocol lifecycle, packaging, and security surface. |
| Port all Docker CLI loader/converter code line-for-line | Rejected. | The code is Go-specific, tied to a legacy schema, and includes reconciliation behavior Kontainer should not copy. Maintaining a full fork would create permanent upstream-drift work. |
| Port the accepted Docker semantics into focused Rust modules | Accepted. | Reimplement only the declared compatibility contract: interpolation, selected merge rules, short-syntax normalization, names/labels, Compose-to-Swarm field mapping, and golden fixtures pinned to Docker CLI releases. |
| Use only Rust Compose crates and call Engine directly | Rejected as a complete answer. | Available crates model YAML but do not provide the complete loader, Swarm conversion, ownership, journal, diff, rollout, or repair behavior. |
| Build a Kontainer reconciler over Bollard/EngineClient | Accepted. | This creates the required pure-Rust, auditable, resumable control-plane boundary while retaining Docker-compatible resource semantics. |

## Compose compatibility contract

### Versioned dialect

Every StackRevision records:

- loaderVersion;
- composeDialect, initially kontainer-swarm-v1;
- Engine negotiated API and capability revision;
- normalization and policy versions;
- canonical plan digest;
- ordered source-file digests.

The initial dialect accepts files with no version or with version 3.x. The version value does not select the parser; it is checked only to reject legacy v1/v2 input that operators could reasonably expect to behave differently. The stack name in the API path is authoritative. A top-level name must match it after normalization or validation fails; it never silently redirects a deployment.

Compatibility means that a source using only fields declared supported below produces materially equivalent Swarm resources to Docker CLI 29.7.2, subject to documented Kontainer safety policy and stronger ownership labels. It does not mean every current Compose Specification field is supported.

### Source bundle

The public request should reference an immutable uploaded bundle rather than server filesystem paths:

~~~text
StackSource
  stackName
  composeFiles[]       ordered relative paths
  bundleRef            content-addressed archive/object
  variables{}          explicit non-secret values
  secretVariableRefs{} opaque secret-store references
  registryCredentialRefs{}
  options
~~~

Rules:

- composeFiles are relative, normalized UTF-8 paths within the bundle;
- reject absolute paths, parent traversal, NULs, devices, sockets, hard links, and symlinks that escape the root;
- cap compressed and expanded bytes, file count, per-file bytes, YAML nodes, depth, aliases, and scalar length;
- resolve config, secret, and env_file paths only inside the bundle;
- never consult the controller process environment or manager host filesystem;
- never return secret-variable values or secret file bytes after admission;
- preserve original source plus spans for diagnostics, but store sensitive bundles encrypted through the approved secret/storage boundary.

An editor-only request is represented as a one-file bundle. Future Git/OCI sources must produce the same immutable StackSource before entering the loader.

### Loader pipeline

The pipeline is deterministic:

1. Parse YAML into a span-preserving value tree. Reject duplicate mapping keys, non-string object keys, unknown tags, multiple top-level documents, and resource-limit violations.
2. Resolve YAML anchors and merge keys within the same document. Detect alias cycles and enforce expansion limits.
3. Interpolate each file independently. Support direct, default, required, and alternative Compose forms plus double-dollar escaping. Interpolate values, not YAML keys.
4. Resolve variables from the request map, then approved secret-variable references, then bundle interpolation env files if explicitly named. Never use ambient agent variables. Undefined ordinary variables are errors by default; compose-compatible warn-and-empty is an explicit option shown in the plan.
5. Validate raw fields before typed deserialization so unknown or unsupported paths cannot disappear through flattening.
6. For the MVP, load exactly one Compose file. The multi-file phase expands short syntax and applies the documented Compose mapping, sequence, command, and unique-resource merge rules before continuing. !reset and !override remain rejected until their behavior is fixture-covered.
7. Resolve each service env_file inside the bundle in listed order; service environment entries override env_file entries. Preserve KEY with no value distinctly until explicit-variable resolution.
8. Select services. MVP rejects profiles and include rather than deploying a surprising subset.
9. Expand short forms for commands, environment, labels, ports, mounts, networks, configs, secrets, durations, and byte values into typed canonical forms.
10. Apply documented defaults and validate cross-references, dependency cycles, mount targets, port collisions, mode/replica combinations, durations, ratios, placement expressions, and external-resource declarations.
11. Convert to StackPlan and run Engine capability, platform, organization-policy, and security validation.
12. Sort order-insensitive collections, retain order-sensitive commands, and compute canonical digests.

All diagnostics use a stable code and include source file, line/column, JSON-style path, severity, message, and remediation. Validation returns all safely discoverable errors in one response rather than stopping at the first typo.

### Supported initial subset

Supported means parsed, normalized, converted, diffed, and tested. Capability-gated means validation can reject it on an older API, unsupported OS, or missing driver.

| Compose area | v2 MVP treatment |
|---|---|
| Top level | services required; networks, volumes, configs, secrets supported; version informational; name must match; x-* retained as opaque metadata and ignored with a warning. |
| Service process | image required; command, entrypoint, environment, env_file, user, working_dir, hostname, init, read_only, stop_grace_period, stop_signal, stdin_open, tty supported. |
| Health | healthcheck test, interval, timeout, retries, start_period, and capability-gated start_interval supported. |
| Metadata/runtime | service labels and container labels supported; logging driver/options, extra_hosts, dns, dns_search, cap_add, cap_drop, sysctls, ulimits, oom_score_adj, and isolation are capability/policy-gated. |
| Ports | short and long syntax; target, published, protocol tcp/udp/sctp when supported, and ingress/host publish mode. Reject ambiguous ranges or collisions. |
| Mounts | named/external volume, bind, and tmpfs mounts supported with policy gates. npipe, image mounts, cluster volumes, subpath, and advanced bind options are capability-gated or deferred until Engine fixtures exist. |
| Networks | default and named overlay networks; driver/options, internal, attachable, labels, and bounded IPAM subnet configuration. External networks require an existing swarm-scoped network. |
| Configs | bundle file source, external/name, labels, service source/target/uid/gid/mode, and capability-gated template driver. Content changes rotate to a new immutable config. |
| Secrets | bundle file source or approved secret-store reference, external/name, labels/driver options, service source/target/uid/gid/mode. Content changes rotate to a new immutable secret. |
| Deploy mode | replicated, global, replicated-job, and global-job when supported by the Engine baseline. replicas is valid only for replicated/job semantics. |
| Deploy policy | endpoint_mode, deploy labels, placement constraints/preferences/max_replicas_per_node, CPU/memory/PID/generic-resource limits/reservations, restart_policy, update_config, and rollback_config. |

### Unsupported-field policy

Strict is the default and the only mode allowed for unattended deployment:

- unknown Compose fields: error;
- known but unsupported fields that affect runtime: error;
- x-* extension fields: retain and warn unless a documented x-kontainer field exists;
- known no-op presentation fields such as container_name or expose: error in strict mode, warn-and-ignore only in interactive compatibility mode;
- security-sensitive fields never become warn-and-ignore.

Initially rejected fields include build, develop/watch, provider, include, extends, profiles, links, external_links, volumes_from, service network_mode, pid, ipc, privileged, devices, device_cgroup_rules, security_opt, shm_size, userns_mode, cgroup_parent/cgroupns_mode, mac_address, pull_policy, models, and host-specific namespace sharing. Some can move to capability-gated support only with explicit conversion, policy, and fixtures.

If build is present alongside image, strict mode still rejects it; Kontainer must not imply the image was built. A future build pipeline may consume it before stack planning.

Compatibility mode is an interactive migration aid. Its warnings are included in the plan, require confirmation, are stored with the revision, and prevent use by deploy tokens/OIDC until policy explicitly allows them.

## Architecture

### Components

| Component | Responsibility |
|---|---|
| SourceStore | Immutable bounded bundles, encryption, content digests, retention, and source provenance. |
| ComposeLoader | Secure parse, interpolation, merge, expansion, field policy, and source diagnostics. |
| StackNormalizer | Compose subset to versioned StackPlan with defaults and canonical ordering. |
| StackPolicy | Authorization, organization rules, security gates, capability checks, and destructive-action approval. |
| RegistryResolver | Credential selection, distribution inspection, digest/platform resolution, and redacted errors. |
| StackInventory | Lists resources by stack UUID/labels, validates bindings, and identifies legacy/suspicious resources. |
| StackPlanner | Three-way desired/current/last-applied comparison and deterministic StackDiff. |
| StackReconciler | Executes journaled convergent steps under a stack lock. |
| RolloutObserver | Event-assisted polling, task/container health, convergence, timeouts, and progress events. |
| OperationStore | Revisions, idempotency, locks/leases, step journal, results, recovery, and audit references. |
| EngineClient | Typed version-aware Docker Engine operations; Bollard types never escape this adapter. |

The controller may use a local Engine only when Info reports an active Swarm with ControlAvailable. A Kontainer deployment role is not proof that the daemon is a functioning manager.

### Normalized desired-state model

StackPlan contains only decisions the reconciler understands:

~~~text
StackPlan
  schemaVersion
  stack { id?, name, namespace, labels }
  services[logicalName] ServicePlan
  networks[logicalName] NetworkPlan | ExternalNetworkRef
  volumes[logicalName] VolumeMountPlan | ExternalVolumeRef
  configs[logicalName] ImmutableFilePlan | ExternalConfigRef
  secrets[logicalName] ImmutableSecretPlan | ExternalSecretRef
  dependencies[]
  imageResolutionPolicy
  prunePolicy
  rolloutPolicy
  sourceDigest
  planDigest
  diagnostics[]
~~~

ServicePlan is an owned, Engine-independent desired spec. It preserves the requested image separately from the resolved digest. Secret values are opaque references. All option/enumeration types have an explicit unknown/unsupported path so a newer daemon cannot silently coerce a value.

The stored canonical plan is the source of diff and rollback. The public DTO is a redacted projection, not serialized Bollard/Moby models.

### Naming, labels, and ownership

Use Docker-compatible names where safe:

- services: stack_service;
- managed networks: stack_network, including stack_default;
- ordinary volume source: stack_volume unless top-level name is explicit;
- immutable configs/secrets: stack_logical_rREV_HASH, truncated with a collision-resistant suffix to the Engine limit;
- external resources: their declared exact name, never scoped.

Every managed service, network, config, and secret receives:

- com.docker.stack.namespace = stack name;
- io.kontainer.managed = true;
- io.kontainer.cluster-id;
- io.kontainer.stack-id = immutable stack UUID;
- io.kontainer.logical-name;
- io.kontainer.revision;
- io.kontainer.plan-digest;
- io.kontainer.resource-digest;
- io.kontainer.created-by-operation when useful for crash recovery.

Services also retain com.docker.stack.image with the requested image, and both service and container labels carry the namespace/stack UUID needed by inventory and task correlation.

The io.kontainer.* and com.docker.stack.* prefixes are reserved; user attempts to set them fail validation. Label size is bounded and no label contains a credential, environment value, config content, secret hash suitable for offline guessing, or full source.

Ownership rules:

- a stack UUID, not its display name, is the ownership authority;
- a same-name unowned object is a conflict, never silently adopted;
- external declarations are references and never gain ownership labels;
- predefined/ingress networks are never owned or deleted;
- deletion requires stack UUID and logical binding to agree;
- namespace-only Docker CLI/v1 resources are legacy, not automatically trusted.

Legacy stacks remain listable. An admin may preview and explicitly adopt resources whose normalized specs match, or explicitly remove the exact namespace-labelled legacy set. Secrets cannot be fully verified because Engine never returns their content; adoption must retain them as opaque external references or require replacement. The v1 compatibility removal route may preserve namespace-only behavior during a documented migration window, but the v2 route never does so implicitly.

### Desired, observed, and last-applied state

Planning uses three inputs:

1. desired: the new normalized StackPlan;
2. observed: a fresh Engine snapshot including IDs and ObjectVersion.Index;
3. lastApplied: the last committed StackRevision and resource bindings.

The diff classifies each logical resource:

- Create;
- UpdateInPlace;
- RotateImmutable;
- ReplaceBlocked or ReplaceApproved;
- RetainExternal;
- RetainOrphan;
- PruneOwned;
- NoChange;
- Conflict;
- Unknown because observation is incomplete.

Canonical comparison removes server-populated timestamps/status, normalizes nil/default forms, sorts order-insensitive sets, preserves order-sensitive arrays, and distinguishes a requested image tag from the manager-resolved digest. A plan records why each field differs.

For a managed field changed both externally and by the new desired plan since lastApplied, default to conflict and require re-plan. For externally drifted fields left unchanged by the new source, the plan explicitly proposes either Converge or Preserve according to the stack's drift policy. It never silently decides during apply.

## API and desktop contract

Recommended v2 endpoints:

| Endpoint | Result |
|---|---|
| POST /api/v2/stacks/validate | Parsed source summary and diagnostics; no Engine mutation required. |
| POST /api/v2/stacks/{name}/plans | Fresh capability/external/image checks plus redacted diff, plan ID, digest, observed versions, expiry, and approval requirements. |
| POST /api/v2/stacks/{name}/operations | 202 with operation ID; body references plan ID, baseRevision, idempotency key, and deploy options. |
| DELETE /api/v2/stacks/{name} | 202 removal operation after a current removal plan/confirmation. |
| GET /api/v2/stack-operations/{id} | Durable phase, progress, per-resource steps, warnings/errors, rollout summary, and result revision. |
| POST /api/v2/stack-operations/{id}/cancel | Cooperative cancel request. |
| POST /api/v2/stacks/{name}/rollback | Forward reconcile to a selected successful revision. |
| POST /api/v2/stacks/{name}/repair | Resume/re-plan actionable partial state under explicit policy. |
| GET /api/v2/stacks/{name}/revisions | Redacted immutable history. |

Plans are immutable and short-lived. Apply rechecks the stack base revision, manager/capability revision, resource versions, external references, and policy. If they changed, it returns plan_stale rather than applying a materially different diff.

Operation events should be available through the existing authenticated real-time transport, with polling as the durable fallback. Disconnecting the desktop does not cancel an operation.

## Planning and dry run

A dry run performs every non-mutating step:

- parse/interpolate/normalize/validate;
- authorization and security policy;
- manager/capability checks;
- external-resource inspection;
- observed stack inventory and ownership validation;
- registry manifest/digest/platform resolution when credentials allow;
- full diff, ordering, rollout estimate, prune list, retained volume list, and warnings.

It does not create resources, pull layers to every node, reserve ports, prove bind paths on future scheduler targets, or guarantee that live versions will remain unchanged. The response states these limits.

The plan contains a server-stored approval record binding principal, request hash, stack/base revision, observed resource versions, destructive actions, policy version, and expiry. Reusing it with different input is an approval_conflict. Destructive preview tokens are one-time or explicitly replay-safe.

## Reconciliation semantics

### Operation state machine

~~~text
queued
  -> acquiring_lock
  -> preflight
  -> resolving_images
  -> applying
  -> observing
  -> pruning
  -> verifying
  -> succeeded | succeeded_with_warnings

any active state
  -> cancel_requested -> cancelled | failed_after_cancel
  -> rollback_requested -> rolling_back -> rolled_back | rollback_failed
  -> failed | partial_failure | timed_out
~~~

Terminal state is never inferred from an HTTP connection. Every transition is transactional in OperationStore and produces a redacted audit event.

Each operation step has pending, claimed, applying, observing, succeeded, skipped, failed, or uncertain state. A step records desired digest, Engine identity/version before and after, attempt count, bounded error detail, and timestamps.

### Deploy/update phase order

1. Acquire the per-cluster/per-stack lock and validate its fencing token.
2. Re-run manager, capability, authorization, base-revision, plan-expiry, and ownership preflight.
3. Refresh all observed Engine versions. A conflict before the first mutation returns plan_stale; it is safe to generate a new plan.
4. Resolve every service image and credential reference. No resource is mutated until all required images/external resources validate.
5. Materialize new immutable configs and secrets in deterministic logical-name order.
6. Create missing managed networks in deterministic order.
7. Create or update services. MVP uses stable lexical order and bounded concurrency; the dependency phase later uses topological waves.
8. Observe the affected services according to the rollout policy. Do not prune old resources before the replacement workload passes its gate.
9. If explicitly requested, prune owned orphan services.
10. Remove unreferenced old config/secret revisions created by this stack, then eligible owned orphan networks. Recheck references immediately before every delete.
11. Perform a full resync, verify desired digests/ownership/convergence, commit the new StackRevision as current, and release the lock.

Mutating calls are normally sequential within a resource class for a small stack and bounded for large stacks. Stable order and explicit maximum concurrency protect the manager and make tests reproducible.

### Service create/update

Service conversion follows the supported Docker mapping but does not construct a sparse update:

- inspect the service immediately before mutation;
- compare the planned ObjectVersion and resource digest;
- overlay every Kontainer-owned supported field onto a fresh full spec;
- clear omitted supported fields to their desired default;
- preserve Engine/server-managed or newer opaque fields outside Kontainer's ownership map;
- preserve ForceUpdate unless the operation is an explicit restart;
- use the current version in ServiceUpdate;
- treat an already matching desired digest as success after uncertain retry.

A version conflict is not blindly retried. If no mutation has occurred, the operation may return plan_stale. After any mutation, it becomes partial_failure/external_change_conflict and requires repair or explicit re-plan.

Deleting a service is convergent: absent is success within a journaled prune/remove step. Creating by stable name is convergent only if the existing object's stack UUID and desired digest match; otherwise it is name_conflict.

### Configs and secrets

Swarm config and secret data are immutable. Docker recommends creating a new version, updating services to use it, and removing the old version ([configs](https://docs.docker.com/engine/swarm/configs/), [secrets](https://docs.docker.com/engine/swarm/secrets/)).

Kontainer therefore:

- gives each changed managed config/secret a deterministic revisioned Engine name;
- keeps the Compose logical name and mount target stable;
- creates the new object before updating any service;
- resolves service references to Engine IDs;
- retains the old object until every referencing service converges;
- removes it only if it is owned by this stack and a fresh cluster-wide reference check is empty;
- never reads secret bytes back, logs them, or returns them;
- uses a keyed digest for secret deduplication/identity so low-entropy data cannot be guessed from public hashes;
- treats a crash after secret creation as recoverable through deterministic name plus stack/revision/operation labels.

An existing same-name secret/config without the stack UUID is never updated or removed. External resources must exist and are referenced exactly.

### Networks

Managed networks default to overlay and are created before services. External user-defined networks must exist with swarm/global scope; predefined local modes are allowed only when the service conversion and policy explicitly support them.

Most network changes cannot be safely updated in place. MVP behavior:

- no change: reuse;
- label-only drift: report and require an explicit repair policy;
- immutable driver/IPAM/internal/attachable/name change: ReplaceBlocked;
- external change: validate but never mutate;
- missing owned unused network: recreate;
- name occupied by unowned network: conflict.

A future approved network replacement can create a revisioned network, roll all services to it, verify connectivity/convergence, and remove the old network. It cannot promise zero interruption and must be a separately confirmed action.

### Volumes and bind mounts

Ordinary named volumes in Swarm are node-local and may be created by the worker when a service task starts. The stack reconciler owns the mount declaration, not one cluster-wide Engine volume object.

Rules:

- scope managed names by default and embed driver/options/labels in the service mount;
- retain named volumes on deploy prune, rollback, and stack removal;
- never claim that manager-only preflight proved a local volume exists on every eligible node;
- use node agents for optional placement-aware checks and report incomplete nodes;
- treat driver/options/name changes as a destructive migration, not an in-place update;
- external volumes are never labelled or deleted;
- bind mounts require an elevated policy because host paths can expose the Docker socket or host root;
- deny Docker socket, root, proc, sys, device, and agent-state paths by default;
- surface that a path can exist on one node and be missing or different on another.

Explicit volume deletion is a future, separately authorized operation with node scope, usage rechecks, backup warning, and no implication that stack rollback restores data.

### Pruning and orphans

Match Docker CLI's safety default: prune is off unless requested. A plan always shows orphans.

When pruneServices is enabled, remove only services with the exact stack UUID that are absent from desired state, and only after desired services pass their rollout gate. Then garbage-collect only unreferenced immutable config/secret revisions and unused owned networks. Never prune:

- external resources;
- predefined/ingress networks;
- namespace-only legacy resources without explicit legacy approval;
- volumes;
- resources owned by another stack UUID;
- a resource whose current references cannot be completely observed.

Failed-operation resources are not invisible orphans. The journal identifies them as pending cleanup and Repair offers the exact convergent steps.

### Stack removal

Removal is a durable operation, not a synchronous loop:

1. lock the stack and snapshot all exact owned resources;
2. show services, configs, secrets, networks, external resources, and retained volumes;
3. remove services first;
4. wait until their current tasks are terminal/absent, within the removal timeout;
5. remove unreferenced owned config/secret revisions;
6. remove owned non-predefined networks, retrying bounded resource-in-use races caused by endpoint cleanup;
7. verify absence and commit a tombstone revision.

No automatic rollback follows deletion. A user can redeploy a retained successful revision, but that is a new operation and does not restore deleted configs/secrets from Engine or deleted volume data. SourceStore retention must therefore be sufficient for an authorized re-deploy.

## Dependency ordering

Swarm has no native service dependency primitive, and Docker stack deploy ignores depends_on during service conversion. Pretending otherwise is unsafe.

MVP strict mode rejects non-empty depends_on. Interactive compatibility mode may warn and deploy concurrently, matching Docker stack behavior.

A later dependency-gate phase may support an acyclic graph with:

- service_started: dependency service reaches task-running convergence;
- service_healthy: every current desired task with a healthcheck is healthy for a stabilization window;
- service_completed_successfully: dependency is a job whose current iteration completed successfully;
- required=false: failure becomes a warning instead of blocking dependants.

Services are applied in topological waves. Failure blocks downstream waves; removal reverses dependency order. Update ordering applies only to services changed by the operation. The feature must document that a later runtime dependency failure does not cause Swarm to stop dependants and that ordered startup is not a substitute for application retry logic.

## Registry authentication and image resolution

Docker's manager normally resolves a tag to a digest for service create/update; if it cannot, workers may resolve the tag independently and run different content ([Swarm service images](https://docs.docker.com/engine/swarm/services/)). The current Moby API schema requires the service's current version to avoid conflicting writes, supports registryAuthFrom, and accepts private-registry credentials through X-Registry-Auth ([Moby Engine API schema](https://github.com/moby/moby/blob/2734510f2b2f2c9421d5583e4c099a6ecb755445/api/swagger.yaml#L13091-L13202)).

Kontainer models:

- requested: the user's registry/repository/tag or digest;
- resolved: canonical repository@digest selected for this operation;
- applied: image value in the accepted service spec;
- observed: image/digest reported by current tasks on nodes.

Policies:

- always, the default: resolve every tagged image for every deployment, so a moved tag can produce a rollout;
- changed: resolve only when the requested string changed; otherwise preserve the currently pinned digest;
- never: accept a tag without registry resolution only under explicit compatibility policy and a prominent non-reproducible warning;
- a supplied digest is verified for syntax and, when reachable, manifest/platform compatibility without changing it.

Registry credentials are server-side opaque references matched to a normalized registry host. The resolver obtains a least-privilege, preferably short-lived pull credential and passes it only for the corresponding service Engine call. Raw credentials never appear in source, plans, revisions, operation records, logs, audit diffs, Tauri, or React.

Preflight resolves all required manifests before mutation and records supported platforms. Placement is checked against image platforms when Engine/model capability permits. Errors distinguish credential unavailable/expired, unauthorized, manifest unknown, rate limited, network/TLS, invalid digest, and platform mismatch. Responses redact registry challenge/token details.

The subsystem does not perform registry catalog browsing, login persistence in a Docker config.json, build, tag, push, vulnerability scanning, or garbage collection.

## Concurrency, idempotency, and locking

### Public idempotency

Every deploy/update/remove/rollback/repair request requires an Idempotency-Key. The durable record is scoped to principal, cluster, endpoint, stack UUID/name, and canonical request hash.

- same key and hash returns the original operation/result;
- same key with a different hash returns idempotency_conflict;
- the record outlives ordinary client retry windows and controller restarts;
- restart/rollback cannot execute twice after an uncertain response;
- retention and maximum key lifetime are explicit configuration.

### Per-stack serialization

Only one mutating operation may own a stack at a time. The lock key is cluster ID plus normalized stack name/UUID.

In the production profile, use a shared transactional database and a renewable row lease or database advisory lock. The operation claim contains owner controller instance, monotonic fencing generation, heartbeat, and expiry. Before each Engine call and before each journal commit, the worker verifies its claim.

SQLite supports the documented single-controller profile only. It cannot provide cross-host HA merely because the database file is durable.

Docker Engine cannot consume Kontainer's fencing token, so a paused old worker could theoretically reach the daemon after lease loss. Reduce this window with short claims, cancellation on database loss, one active operation task per claim, version checks immediately before calls, and idempotent post-call verification. Do not market it as perfect external fencing.

Direct service mutations on stack-owned services acquire the same stack lock. Scale/env/image changes are recorded as explicit runtime overrides or drift according to the selected product policy; restart is an ephemeral operation; service rollback marks the stack drifted. No service endpoint may bypass resource bindings, object versions, or audit.

### Step idempotency and uncertain outcomes

The worker provides at-least-once step execution with convergent verification:

| Step | Resume check |
|---|---|
| Create named resource | Inspect deterministic name; require matching stack UUID and desired/operation digest, then bind the existing ID. |
| Update service | Inspect; matching desired digest is success, mismatching newer ObjectVersion is conflict. |
| Create secret | Inspect deterministic name and operation/revision labels because content cannot be read back. |
| Delete | Absence is success; presence must still match ownership and have no disallowed references. |
| Observe rollout | Reconstruct entirely from current service/tasks plus journal deadline; Engine events are only hints. |
| Commit revision | Transactional compare-and-set from the expected base revision. |

Transport timeout after an Engine request is uncertain, not automatically failed. Inspect before retrying.

## Rollout observation and convergence

Use Engine events to wake observers and invalidate caches, plus bounded polling/full resync as the source of truth. Event disconnect, controller restart, or missed events must not lose an operation.

Convergence is mode-specific:

- replicated: the requested number of current-spec desired tasks are running, no current attempt is terminally failed/rejected, service update is not paused, and the set remains stable for the configured monitor window;
- global: each currently eligible active node has one current-spec running task; zero eligible nodes becomes no_eligible_nodes rather than an endless apparent success;
- replicated job: the current job iteration reaches total successful completions with no unrecoverable failed slot;
- global job: current eligible nodes have successful completion for the current iteration;
- zero-replica replicated service: converged when no current desired-running tasks remain, not unhealthy merely because zero is not positive.

UpdateStatus completed, paused, rollback_started, rollback_paused, and rollback_completed refine the result but do not replace task observation.

For a service with a healthcheck, the default production gate requires current task containers to report healthy after running. Manager task state alone cannot report container health on remote workers, so node-agent observations must be bounded and carry completeness. If an agent is unavailable, strict health-gated deployment is unknown/partial, not healthy. A policy may choose task-running-only with a recorded warning.

Unschedulable placement, pull failure, rejected tasks, crash loops, unhealthy containers, missing volume/bind path, unavailable node, and manager quorum loss receive distinct stable codes. The observer includes the latest relevant task attempts without unbounded task history.

Default rollout timeout should be finite and configurable per operation, with service/wave and overall caps. A client request timeout is separate; operations continue durably. A timeout applies the declared failure policy and never silently turns into success because tasks converge later.

## Cancellation, rollback, and recovery

### Cancellation

Cancellation is cooperative:

- set cancel_requested durably;
- stop before the next mutation or polling cycle;
- cancel in-flight HTTP where possible, but assume the daemon may have accepted it;
- inspect and journal the outcome before terminalizing;
- do not prune after cancellation;
- do not imply that prior mutations were undone.

An operation cancelled before mutation is cancelled. After mutation it is cancelled_with_changes or partial_failure. Automatic rollback on cancel is a separate requested policy.

### Service rollback versus stack rollback

Swarm service rollback uses the service's single PreviousSpec and can be triggered by update failure policy or an explicit service action. It cannot remove a newly created service, restore a removed service/network, or coordinate config/secret revisions.

Stack rollback is a forward reconcile to the selected last successful StackRevision:

1. create/reuse that revision's immutable configs/secrets;
2. restore networks if possible or block on immutable conflict;
3. create/update previous services using fresh versions;
4. observe convergence;
5. remove resources introduced only by the failed revision when safe;
6. commit a new revision recording rollback provenance.

Automatic stack rollback is opt-in. If any service has already performed a Swarm automatic rollback, the observer first snapshots the resulting actual spec and plans from it. Rollback can itself fail and yields rollback_failed with repair steps; it is never described as atomic.

### Failure and partial results

Multi-resource mutation is not transactional. Operation results contain:

- overall outcome;
- whether any Engine mutation may have occurred;
- exact succeeded, failed, skipped, uncertain, and retained steps;
- last successful revision;
- current observed completeness and convergence;
- rollback attempt/result;
- safe retry/repair actions;
- redacted error chain and audit ID.

Default failure policy is stop: do not start new independent mutations after a hard failure, but finish necessary observation of uncertain in-flight calls. Do not prune. continue is allowed only for explicitly independent warnings. autoRollback invokes the stack rollback flow.

### Controller crash and resume

OperationStore persists before/after every side effect. On startup or lease expiry, a controller:

1. claims an abandoned non-terminal operation;
2. validates that its stack lock/fencing generation is current;
3. refreshes manager/capabilities and full owned inventory;
4. verifies every completed/uncertain step from Engine state;
5. resumes the first incomplete convergent step or records an ownership/version conflict;
6. reconstructs rollout deadlines from persisted absolute times;
7. completes, rolls back, or returns a repairable partial result.

Operations do not depend on one process's memory or one Engine event stream. If manager quorum is unavailable, keep the operation waiting/retriable only within its persisted deadline, then fail with quorum_unavailable; existing workloads may still run.

## Durable data and history

The logical schema is:

~~~text
stacks
  id, cluster_id, name, state, generation
  current_revision_id, last_successful_revision_id

stack_revisions
  id, stack_id, sequence, parent_id, source_kind
  source_ref, source_digest, loader/policy/capability versions
  encrypted_plan_ref, redacted_summary, created_by, created_at

stack_operations
  id, stack_id, kind, state, phase, requested_revision_id, base_revision_id
  request_hash, idempotency_key, principal_id
  lease_owner, fencing_generation, lease_expires_at
  cancel_requested_at, absolute_deadline, result, timestamps

operation_steps
  operation_id, ordinal, resource_kind, logical_name, action
  desired_digest, engine_id/name, expected/observed_version
  state, attempts, error_code, timestamps

resource_bindings
  stack_id, resource_kind, logical_name
  engine_id/name, ownership_token, desired_digest, last_version

audit_events
  actor, action, target, request/operation IDs
  redacted_diff, decision, result, timestamp
~~~

Production multi-controller mode requires a shared transactional database. The local SQLite profile is acceptable for parity/development with one controller and explicit non-HA status, consistent with [cluster-communication.md](./cluster-communication.md).

Store enough encrypted source/plan material to redeploy an authorized revision. Secret bytes belong in the secret store, referenced from revisions; registry credentials are never revision data. Config content should be treated as sensitive even when Compose calls it config. Backups, retention, encryption keys, migrations, and deletion policy must be documented before history is advertised as recovery.

## Security and audit

Stack deployment has near-root consequences because service specs can mount host paths, add capabilities, publish ports, and run arbitrary images.

Required controls:

- separate validate, deploy, prune, rollback, adopt-legacy, and remove permissions, optionally scoped by stack/repository/environment;
- mandatory re-authorization when a queued operation starts, with the policy decision version persisted;
- policy denial for Docker socket/root/proc/sys/device binds, privileged/host namespaces, unsafe capabilities, untrusted registries, mutable latest tags, unsigned images, or host publishing as configured;
- bundle parsing limits and archive/path traversal defenses;
- no ambient environment-variable interpolation;
- secret references resolved server-side and values redacted from diagnostics, plan hashes exposed to clients, logs, tracing, errors, and audit;
- registry host normalization and prevention of credential forwarding to a different host after redirects;
- outbound registry allow-list, TLS validation, bounded response bodies, deadlines, and SSRF protection;
- user labels cannot overwrite ownership labels;
- external-resource adoption and deletion require exact previews and elevated confirmation;
- operation event subscriptions enforce the same stack authorization as polling;
- Engine errors are mapped to stable domain codes and redact daemon paths, headers, and registry challenges.

Audit records include principal/auth method, cluster and stack UUID, source/plan digest, base/result revision, policy and capability versions, requested options, redacted field-level diff, approval/confirmation identity, operation/step outcomes, rollback, timestamps, and client origin. They do not include environment values, config/secret content, registry auth, raw JWT/OIDC claims, or full daemon messages.

## Delivery phases

### Phase 0: corrected v1 parity

- Derived namespace-labelled stack list/detail.
- Journaled stack removal with volumes retained.
- Existing service scale/restart/rollback/env/image behavior through one safe service patch path.
- Repair all HTTP/Tauri response and method mismatches.
- Version conflicts, idempotency, operation progress, and audit for mutations.
- Legacy stacks visibly classified; no automatic adoption.

There is still no Compose deployment requirement for strict v1 parity.

### Phase 1: v2 MVP deployment

- One Compose file plus an immutable referenced-file bundle.
- Explicit variables; no ambient environment.
- Strict supported subset and field-level diagnostics.
- Normalize, validate, registry-resolve, dry-run/diff, and short-lived plan approval.
- First deploy/update/remove through durable operations.
- Strong naming/ownership, service/network/config/secret handling, immutable rotation, volume retention.
- Per-stack lock, idempotency, optimistic concurrency, crash resume, partial results.
- Task-running rollout gates, bounded timeout/cancel, manual rollback to a successful revision.
- Desktop deploy wizard, rollout view, retained-resource removal preview, and history.
- Single-controller SQLite profile and shared-database-ready interfaces.

### Phase 2: production completion

- Shared-database multi-controller claims/failover.
- Node-agent container-health gates and placement-aware volume/bind checks.
- Multi-file Compose merge with fixtures, then selected !reset/!override support.
- Optional dependency waves and health/completion conditions.
- Automatic stack rollback policy.
- Approved network replacement workflow.
- Explicit runtime-override policy for direct edits of stack-owned services.
- Legacy adoption/repair workflow.
- Larger compatibility and chaos matrix.

### Deferred Git, GitOps, registry, and build work

- GitHub App/OIDC deploy policy, deploy tokens, custom GitHub Action, and repository provenance.
- Git/OCI source fetch, signed commits/tags/artifacts, branch-to-stack policy, webhook reconciliation, drift loops, and promotion environments.
- Dockerfile/BuildKit build, cache, attestations, signing, SBOM, push, and image promotion.
- GHCR installation-token automation, registry browsing, catalog/tag UI, vulnerability scanning, and garbage collection.
- Scheduled, canary, blue/green, traffic-aware, and database-snapshot deployments.
- Continuous GitOps convergence and environment cloning.

These features must feed the same StackSource, StackPlan, operation, and audit boundaries rather than creating a second deploy path.

## Testing and compatibility strategy

### Unit, property, and fuzz tests

- YAML duplicate keys, anchors, merge cycles, aliases, tags, depth/node/size limits, Unicode, numbers/booleans, and malformed archives.
- Every interpolation operator, escaping, unset/empty distinction, recursive expressions, and value-only behavior.
- Short/long syntax normalization for environment, labels, ports, mounts, networks, configs, secrets, durations, bytes, and healthchecks.
- Cross-reference errors, dependency cycles, port/mount conflicts, mode/replica rules, and reserved labels.
- Canonicalization is deterministic across map order; normalize(normalize(x)) is stable.
- Diff identities, three-way drift conflicts, default/nil equivalence, requested versus resolved image, and unknown fields.
- Operation and step state-machine transition properties; invalid transitions fail.
- Idempotency and crash points before/after every simulated Engine call.

Fuzz targets must have memory/time limits and regression seeds.

### Docker CLI compatibility fixtures

Pin Docker CLI v29.7.2 as the first oracle:

- import selected Apache-licensed loader/converter fixture inputs with attribution, or generate golden outputs in CI tooling;
- compare docker stack config merge/interpolation output for the supported overlap;
- compare created ServiceSpec, network options, labels, scoped names, mounts, config/secret references, and image-resolution behavior on a real daemon;
- prove each intentional difference, such as strong ownership, immutable rotation, preflight-before-prune, and stable dependency policy, in a named fixture;
- rerun against each Docker CLI release considered for a dialect upgrade. Compatibility never floats with latest.

Also consume the Compose Specification conformance corpus where it tests syntax Kontainer claims to support ([conformance tests](https://github.com/compose-spec/conformance-tests)).

### Engine integration matrix

Run against the selected minimum Engine/API and the current Engine 29.7/API 1.55 baseline; Docker's current API matrix documents 29.7 and negotiation behavior ([Engine API](https://docs.docker.com/reference/api/engine/)).

Cover:

- new and existing single-node stacks;
- multi-node Linux Swarm, mixed eligible/ineligible nodes, drained/down nodes;
- replicated/global/job modes and zero replicas;
- private registry, moved tags, digest pins, multi-platform manifests, expired credentials, and registry outage;
- update/rollback policies including pause/continue/rollback and start-first/stop-first;
- config/secret rotation and in-use cleanup;
- external and conflicting resources;
- local/external volumes, bind path absence, and volume retention;
- host/ingress ports and collisions;
- manager loss/quorum loss and API version/capability gates;
- Windows-only fields only if Windows becomes supported.

### Fault, recovery, and security tests

Inject failure:

- before and after every Engine call and journal commit;
- after Engine acceptance but before response;
- during config/secret creation, service update, rollout, prune, and removal;
- controller process kill, lease expiry, shared DB disconnect/failover, and two competing controllers;
- external Docker CLI mutation causing ObjectVersion conflict;
- event-stream disconnect and stale/reordered events;
- task churn, crash loop, unhealthy container, node-agent timeout, and network endpoint cleanup lag;
- cancellation and deadline at every phase;
- rollback failure and repeated repair.

Security cases include archive traversal/symlink escape, YAML alias bombs, SSRF registry names/redirects, credential host confusion, malicious labels, Docker socket binds, secret leakage snapshots, log/error redaction, authorization changes while queued, and legacy-resource spoofing.

### Scale and soak

Benchmark representative 10/100/500-service plans, thousands of task-history records, large env/label maps, registry latency, and 24-hour event reconnect/rollout observation. Enforce bounded Engine concurrency, memory, journal growth, event buffers, polling, and per-operation logs according to [performance.md](./performance.md).

## Acceptance checklist

### Compatibility and validation

- [ ] The supported Compose dialect and Engine capability matrix are versioned and published.
- [ ] Unknown and unsupported fields follow strict/warn policy with source spans and stable codes.
- [ ] The controller never reads ambient environment or arbitrary host paths while loading a stack.
- [ ] Interpolation, short syntax, defaults, references, canonicalization, and supported multi-file behavior have Docker/Compose golden fixtures.
- [ ] Every accepted field is either converted into desired state or explicitly retained metadata; none disappears silently.
- [ ] compose_spec/parser dependency, maintenance, license, advisories, duplicate-key behavior, and resource limits pass review.

### Planning and ownership

- [ ] Dry run is mutation-free and returns a deterministic digest, complete diff, warnings, versions, expiry, and approval requirements.
- [ ] Apply rejects stale plan/base revision/capability/resource versions.
- [ ] Managed resources carry standard namespace plus immutable Kontainer stack UUID labels; reserved labels cannot be overridden.
- [ ] Same-name unowned resources conflict; external and predefined resources are never owned/deleted.
- [ ] Legacy namespace-only stacks are classified, previewed, and never silently adopted.
- [ ] No-change deploy makes no Engine update and causes no task rollout.

### Resource reconciliation

- [ ] Service update preserves ForceUpdate and opaque server-managed fields while clearing omitted owned fields correctly.
- [ ] Config/secret content changes rotate, update services, converge, then garbage-collect only unreferenced owned versions.
- [ ] Networks are validated by scope; immutable drift blocks unless a separately approved replacement exists.
- [ ] Named/external volume semantics and node locality are explicit; stack removal retains volumes.
- [ ] Registry credentials stay server-side; requested, resolved, applied, and observed images remain distinct.
- [ ] Prune is opt-in, occurs after successful rollout, and touches only exact owned safe resources.

### Operations and recovery

- [ ] Every mutation has durable idempotency and returns one stable operation ID across retries.
- [ ] Per-stack locking serializes stack deploy/remove/rollback/repair and direct mutations of owned services.
- [ ] All Engine calls have bounded deadlines and uncertain outcomes are inspected before retry.
- [ ] Operation/step state transitions and side-effect intent/result are durably journaled.
- [ ] A controller can be killed at every injected crash point and another controller safely resumes or returns an actionable conflict.
- [ ] Partial failure names every succeeded/failed/skipped/uncertain step and never reports plain success.
- [ ] Cancellation is cooperative and accurately reports whether changes remain.

### Rollout and rollback

- [ ] Replicated, global, zero-replica, replicated-job, and global-job convergence is fixture-tested.
- [ ] Paused/failed/automatic rollback states, task churn, healthchecks, and unschedulable services map to stable results.
- [ ] Event loss triggers full resync; operations never depend on events alone.
- [ ] Rollout and removal have finite persisted deadlines independent of the client connection.
- [ ] Stack rollback is a forward reconcile to a stored revision and is distinguished from service PreviousSpec rollback.
- [ ] Rollback failure is journaled and repairable without deleting retained volumes/external resources.

### Security, UX, and observability

- [ ] RBAC separately gates validate/deploy/prune/remove/rollback/adopt and is rechecked at execution.
- [ ] Bundle/path/YAML/registry/security policy tests pass; dangerous mounts/capabilities are denied by default.
- [ ] Secret, config-sensitive, environment, registry, and auth material are absent from DTOs/logs/errors/audit.
- [ ] Audit events contain actor, target, redacted diff, policy, operation, result, and rollback provenance.
- [ ] Desktop supports validate, diff, approval, rollout, cancellation, partial repair, history, and honest removal copy.
- [ ] Stack list uses convergence/completeness and last successful operation, not v1 replica heuristics.

## Issue-ready open decisions

1. **Select the Compose YAML parser.** Compare maintained candidates for YAML version behavior, duplicate-key rejection, anchors/merge tags, spans, alias limits, safety, maintenance, license, and fuzz results. Recommended default: do not accept compose_spec's archived serde_yaml dependency transitively without mitigation.
2. **Approve compose_spec 0.3 as a loader-boundary dependency.** Complete field-gap, parser, MPL-2.0, MSRV, advisory, and fixture review. Recommended default: use only as provisional typed syntax help, never as the complete loader or domain model.
3. **Freeze kontainer-swarm-v1 syntax.** Approve the exact supported/rejected field matrix and Engine minimum. Recommended default: API 1.41 full baseline, one-file strict MVP, jobs capability-gated.
4. **Choose multi-file compatibility.** Decide whether Phase 2 follows modern Compose merge, Docker stack's legacy merge, or a documented subset, including !reset/!override. Recommended default: modern documented merge, versioned separately, after oracle fixtures.
5. **Define variable sources and secrecy.** Specify explicit request values, interpolation env files, secret references, undefined behavior, audit/redaction, and whether values may be retained encrypted. Recommended default: no ambient env; undefined is error; secret refs only for sensitive values.
6. **Approve dangerous service policy.** Define registry allow-lists, tag/digest rules, bind paths, capabilities, host publish, Windows isolation, and break-glass roles. Recommended default: deny host-root/socket/namespace exposure.
7. **Define stack adoption/removal migration.** Specify namespace-only legacy discovery, verifiable adoption, opaque secrets, compatibility route lifetime, and elevated removal confirmation. Recommended default: no implicit adoption; explicit preview and admin action.
8. **Choose prune defaults and orphan retention.** Decide service prune default, old config/secret retention, network cleanup, failed-operation cleanup, and confirmation freshness. Recommended default: prune off, show all orphans, never volumes.
9. **Define direct mutations of stack-owned services.** Choose reject, persistent runtime overrides, or desired-revision edits for scale/env/image; define restart and service rollback drift. Recommended default: shared stack lock plus visible explicit override, never silent source mutation.
10. **Set image resolution and registry custody policy.** Decide always/changed default, best-effort never mode, credential backend/refresh, host matching, and multi-platform validation. Recommended default: always for interactive deploy, digest-pinned revision, short-lived credential refs.
11. **Approve immutable config/secret naming.** Select length/collision scheme, keyed digest policy, rotation retention, and crash-recovery labels. Recommended default: deterministic revisioned names and keyed secret digests.
12. **Choose production OperationStore and lease semantics.** Select PostgreSQL or another shared transactional store, advisory/row locks, lease duration, fencing behavior, migrations, and SQLite profile limits. Recommended default: PostgreSQL production, SQLite single-controller only.
13. **Set idempotency/history retention.** Define key TTL, revision/source encryption, maximum bundle/history size, tombstones, deletion, backup, and recovery guarantees. Recommended default: idempotency longer than all retry windows; retain last successful revisions according to explicit quota.
14. **Define rollout success and health completeness.** Set stabilization windows, healthcheck fan-out, global zero-eligible behavior, job semantics, timeouts, and incomplete-node policy. Recommended default: current-spec tasks plus health when declared; unknown is not success.
15. **Choose stack failure policy.** Specify stop/continue/autoRollback defaults and interaction with Compose update_config failure_action. Recommended default: stop and preserve partial evidence; automatic stack rollback opt-in.
16. **Specify cancellation semantics in the public API.** Define terminal states, cancel during uncertain Engine calls, optional rollback-on-cancel, and UI language. Recommended default: cooperative cancel with honest changed-state result.
17. **Design dependency gates.** Decide whether/when depends_on conditions become topological rollout gates and how remote health is measured. Recommended default: strict rejection in MVP; compatibility warning only; staged feature later.
18. **Approve network replacement policy.** Define which drift is repairable, confirmation, revisioned naming, outage language, and rollback. Recommended default: block immutable changes in MVP.
19. **Standardize operation/error/event schemas.** Align HTTP, Tauri, React, WebSocket, audit, and the wider v2 resource API on operation envelopes, stable error codes, redaction, and partial results.
20. **Define Docker compatibility governance.** Pin oracle versions, fixture licensing/attribution, minimum/current Engine CI, intentional-difference ledger, and criteria for a new dialect version.

## Definition of done

Stack deployment is ready when an authorized user can upload a supported source bundle, receive a trustworthy validation and diff, apply it once despite retries, watch an honest bounded rollout, survive controller/manager interruptions, inspect every partial side effect, safely repair or reconcile to a previous revision, and remove only the stack's owned disposable resources while retaining data volumes and every external resource.

No success response may depend on hidden Docker CLI behavior, process memory, silent unsupported fields, a namespace label alone, or the desktop remaining connected.
