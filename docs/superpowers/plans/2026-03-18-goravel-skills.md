# Goravel Skills Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create 7 Claude Code skills (with 24 supporting reference files) that teach Claude to write idiomatic Goravel code, distilled from the official Goravel documentation.

**Architecture:** Git subtree pulls Goravel docs into `docs/goravel/`. Each skill is a directory under `.claude/skills/` with a `SKILL.md` entrypoint (~200 lines) and supporting reference files for advanced content. Skills are generic Goravel knowledge; project-specific conventions stay in CLAUDE.md.

**Tech Stack:** Claude Code skills (Markdown with YAML frontmatter), Git subtree

**Spec:** `docs/superpowers/specs/2026-03-18-goravel-skills-design.md`

**Content rules (apply to all skill-writing tasks):**
- Imperative tone ("Use `facades.Route().Get()` to..." not "You can...")
- Code-first: Go snippets lead, explanation follows
- ~200 lines max per SKILL.md; push comprehensive details to reference files
- Pin to Goravel v1.17 (the version in `server/go.mod`). If the subtree docs describe APIs not available in v1.17, note the minimum version required.
- Cross-reference other `goravel-*` skills instead of duplicating content (e.g., "See goravel-security skill for auth middleware details")
- Assume Claude is smart — skip explanations of general programming concepts

---

### Task 1: Add Goravel Docs Subtree

**Files:**
- Create: `docs/goravel/` (subtree from github.com/goravel/docs)

- [ ] **Step 1: Add the subtree**

```bash
git subtree add --prefix=docs/goravel https://github.com/goravel/docs.git master --squash
```

Expected: Creates `docs/goravel/` with full Goravel docs repo. The `en/` subdirectory contains the 48 source markdown files we'll distill.

- [ ] **Step 2: Verify the subtree**

```bash
ls docs/goravel/en/
```

Expected: Directories including `architecture-concepts/`, `database/`, `digging-deeper/`, `getting-started/`, `orm/`, `security/`, `testing/`, `the-basics/`

- [ ] **Step 3: Commit is automatic with subtree add**

The `git subtree add` command creates its own commit. Verify with:

```bash
git log --oneline -1
```

Expected: A squash commit message about adding the subtree.

---

### Task 2: Create Skill Directory Structure

**Files:**
- Create: `.claude/skills/goravel-foundations/` (directory)
- Create: `.claude/skills/goravel-http/` (directory)
- Create: `.claude/skills/goravel-data/` (directory)
- Create: `.claude/skills/goravel-security/` (directory)
- Create: `.claude/skills/goravel-async/` (directory)
- Create: `.claude/skills/goravel-tooling/` (directory)
- Create: `.claude/skills/goravel-integrations/` (directory)

- [ ] **Step 1: Create all skill directories**

```bash
mkdir -p .claude/skills/goravel-foundations
mkdir -p .claude/skills/goravel-http
mkdir -p .claude/skills/goravel-data
mkdir -p .claude/skills/goravel-security
mkdir -p .claude/skills/goravel-async
mkdir -p .claude/skills/goravel-tooling
mkdir -p .claude/skills/goravel-integrations
```

- [ ] **Step 2: Verify structure**

```bash
ls .claude/skills/
```

Expected: 7 `goravel-*` directories listed.

---

### Task 3: Generate goravel-foundations Skill

**Files:**
- Create: `.claude/skills/goravel-foundations/SKILL.md`
- Create: `.claude/skills/goravel-foundations/service-container.md`
- Create: `.claude/skills/goravel-foundations/service-providers.md`
- Create: `.claude/skills/goravel-foundations/facades.md`

**Source docs to read and distill:**
- `docs/goravel/en/architecture-concepts/service-container.md`
- `docs/goravel/en/architecture-concepts/service-providers.md`
- `docs/goravel/en/architecture-concepts/facades.md`
- `docs/goravel/en/architecture-concepts/request-lifecycle.md`
- `docs/goravel/en/getting-started/configuration.md`
- `docs/goravel/en/getting-started/directory-structure.md`
- `docs/goravel/en/getting-started/installation.md`
- `docs/goravel/en/getting-started/compile.md`
- `docs/goravel/en/getting-started/packages.md`

- [ ] **Step 1: Read all source docs**

Read every file listed above. Take note of core patterns, key APIs, and the 80% use cases.

- [ ] **Step 2: Write SKILL.md**

Create `.claude/skills/goravel-foundations/SKILL.md` with:

```markdown
---
name: goravel-foundations
description: Use when working with Goravel service container, service providers, facades, configuration, or application bootstrap
---
```

Body should contain:
- **Overview:** 1-2 sentences on what Goravel's foundation layer does
- **Core Patterns:** Distilled, code-first coverage of:
  - Registering a service provider (Register + Boot methods)
  - Binding into the service container (Bind, Singleton, Instance)
  - Using facades to access services
  - Configuration via `config/` files and `facades.Config()`
  - Request lifecycle overview (bootstrap → service providers → router → response)
  - Directory structure overview
- **Reference Files:** Index pointing to the 3 supporting files

**Content rules:**
- Imperative tone ("Use `app.Bind()` to..." not "You can bind using...")
- Code-first: Go snippets lead, explanation follows
- ~200 lines max for SKILL.md
- Assume Claude is smart — skip what it already knows about DI and config patterns

- [ ] **Step 3: Write service-container.md**

Comprehensive reference for the service container. Distill from `docs/goravel/en/architecture-concepts/service-container.md`. Cover:
- All binding methods: `Bind`, `BindWith`, `Singleton`, `Instance`
- Contextual binding
- Tagging
- Resolving: `Make`, `MakeWith`
- Method injection

- [ ] **Step 4: Write service-providers.md**

Comprehensive reference for service providers. Distill from `docs/goravel/en/architecture-concepts/service-providers.md`. Cover:
- `Register()` vs `Boot()` lifecycle
- Deferred providers
- Registering providers in `config/app.go`
- Writing a custom provider

- [ ] **Step 5: Write facades.md**

Comprehensive reference for facades. Distill from `docs/goravel/en/architecture-concepts/facades.md`. Cover:
- Full list of available facades and their underlying interfaces
- How facades work (resolving from container)
- Creating custom facades

- [ ] **Step 6: Commit**

```bash
git add .claude/skills/goravel-foundations/
git commit -m "feat: add goravel-foundations skill — service container, providers, facades, config"
```

---

### Task 4: Generate goravel-http Skill

**Files:**
- Create: `.claude/skills/goravel-http/SKILL.md`
- Create: `.claude/skills/goravel-http/routing-reference.md`
- Create: `.claude/skills/goravel-http/validation-rules.md`
- Create: `.claude/skills/goravel-http/request-response.md`

**Source docs to read and distill:**
- `docs/goravel/en/the-basics/routing.md`
- `docs/goravel/en/the-basics/controllers.md`
- `docs/goravel/en/the-basics/middleware.md`
- `docs/goravel/en/the-basics/request.md`
- `docs/goravel/en/the-basics/response.md`
- `docs/goravel/en/the-basics/validation.md`
- `docs/goravel/en/the-basics/session.md`
- `docs/goravel/en/the-basics/views.md`

- [ ] **Step 1: Read all source docs**

Read every file listed above. Focus on routing patterns, middleware chains, request/response API, and validation rules.

- [ ] **Step 2: Write SKILL.md**

Create `.claude/skills/goravel-http/SKILL.md` with:

```markdown
---
name: goravel-http
description: Use when building HTTP endpoints, routes, controllers, middleware, request handling, response building, or validation in Goravel
---
```

Body should contain:
- **Overview:** Goravel's HTTP layer
- **Core Patterns:**
  - Defining routes (`facades.Route().Get()`, `.Post()`, etc.)
  - Route groups with prefix and middleware
  - Writing a controller struct with handler methods
  - Applying middleware (global, group, route-level)
  - Reading request data (`ctx.Request().Input()`, `ctx.Request().Query()`, etc.)
  - Building responses (`ctx.Response().Success().Json()`, etc.)
  - Basic validation (`requests.Validate()`)
- **Reference Files:** Index pointing to 3 supporting files

~200 lines max. Code-first. Imperative tone.

- [ ] **Step 3: Write routing-reference.md**

Full routing API. Cover: route methods, named routes, route parameters, resource routes, rate limiting, route groups, fallback routes, CORS.

- [ ] **Step 4: Write validation-rules.md**

Full validation reference. Cover: all built-in rules, custom rules, form request objects, error messages, conditional validation.

- [ ] **Step 5: Write request-response.md**

Full request and response API. Cover: all request methods (input, query, file, header, cookie, IP), all response types (JSON, string, file, download, redirect, stream), file uploads, cookies, sessions.

- [ ] **Step 6: Commit**

```bash
git add .claude/skills/goravel-http/
git commit -m "feat: add goravel-http skill — routing, controllers, middleware, validation"
```

---

### Task 5: Generate goravel-data Skill

**Files:**
- Create: `.claude/skills/goravel-data/SKILL.md`
- Create: `.claude/skills/goravel-data/orm-methods.md`
- Create: `.claude/skills/goravel-data/query-builder.md`
- Create: `.claude/skills/goravel-data/migrations-reference.md`
- Create: `.claude/skills/goravel-data/relationships-advanced.md`

**Source docs to read and distill:**
- `docs/goravel/en/database/getting-started.md`
- `docs/goravel/en/database/queries.md`
- `docs/goravel/en/database/migrations.md`
- `docs/goravel/en/database/seeding.md`
- `docs/goravel/en/orm/getting-started.md`
- `docs/goravel/en/orm/relationships.md`
- `docs/goravel/en/orm/factories.md`

- [ ] **Step 1: Read all source docs**

Read every file listed above. Focus on model definition, CRUD operations, query builder syntax, and relationship types.

- [ ] **Step 2: Write SKILL.md**

Create `.claude/skills/goravel-data/SKILL.md` with:

```markdown
---
name: goravel-data
description: Use when working with Goravel ORM models, database queries, migrations, seeding, or relationships
---
```

Body should contain:
- **Overview:** Goravel's data layer (ORM + database)
- **Core Patterns:**
  - Defining a model struct (with `orm.Model` embed, `TableName()`, `Connection()`)
  - Basic CRUD: `facades.Orm().Query().Create()`, `.Find()`, `.Update()`, `.Delete()`
  - Running a migration (`facades.Artisan().Call("migrate")`)
  - Creating a migration file
  - Defining a HasOne/HasMany/BelongsTo relationship
  - Basic seeding
- **Reference Files:** Index pointing to 4 supporting files

~200 lines max. Code-first. Imperative tone.

- [ ] **Step 3: Write orm-methods.md**

Full ORM API reference. Cover: all query methods (Find, First, Get, Pluck, Count, Max, Min, Sum, Avg), Create, Save, Update, Delete, ForceDelete, Restore, soft deletes, scopes, events/observers, serialization.

- [ ] **Step 4: Write query-builder.md**

Full query builder reference. Cover: selects, wheres (Where, OrWhere, WhereIn, WhereBetween, WhereNull, WhereRaw), joins, ordering, grouping, having, limit/offset, unions, raw queries, aggregates, chunking, pagination.

- [ ] **Step 5: Write migrations-reference.md**

Full migrations reference. Cover: creating migrations, all column types (String, Integer, BigInteger, Boolean, Date, DateTime, Timestamp, Text, Json, Uuid, etc.), column modifiers (Nullable, Default, Unsigned, Index, Unique, Primary), indexes, foreign keys, renaming/dropping columns and tables.

- [ ] **Step 6: Write relationships-advanced.md**

Full relationships reference. Cover: HasOne, HasMany, BelongsTo, ManyToMany (with pivot), HasOneThrough, HasManyThrough, polymorphic relationships (MorphOne, MorphMany, MorphToMany), eager loading (With), lazy eager loading, relationship queries, touching parent timestamps.

- [ ] **Step 7: Commit**

```bash
git add .claude/skills/goravel-data/
git commit -m "feat: add goravel-data skill — ORM, queries, migrations, relationships"
```

---

### Task 6: Generate goravel-security Skill

**Files:**
- Create: `.claude/skills/goravel-security/SKILL.md`
- Create: `.claude/skills/goravel-security/auth-guards.md`
- Create: `.claude/skills/goravel-security/authorization.md`
- Create: `.claude/skills/goravel-security/encryption-hashing.md`

**Source docs to read and distill:**
- `docs/goravel/en/security/authentication.md`
- `docs/goravel/en/security/authorization.md`
- `docs/goravel/en/security/encryption.md`
- `docs/goravel/en/security/hashing.md`

- [ ] **Step 1: Read all source docs**

Read every file listed above. Focus on guard configuration, JWT usage, gates/policies, and hashing API.

- [ ] **Step 2: Write SKILL.md**

Create `.claude/skills/goravel-security/SKILL.md` with:

```markdown
---
name: goravel-security
description: Use when implementing authentication, authorization, encryption, or hashing in Goravel
---
```

Body should contain:
- **Overview:** Goravel's security layer
- **Core Patterns:**
  - Configuring auth guards in `config/auth.go`
  - Generating and parsing JWT tokens (`facades.Auth().Parse()`, `.User()`)
  - Defining authorization gates (`facades.Gate().Define()`)
  - Using policies for model authorization
  - Hashing passwords (`facades.Hash().Make()`, `.Check()`)
- **Reference Files:** Index pointing to 3 supporting files

~200 lines max. Code-first. Imperative tone.

- [ ] **Step 3: Write auth-guards.md**

Full auth reference. Cover: guard configuration, JWT methods (Login, Parse, User, Refresh, Logout), custom guards, multi-auth, token refresh flows, middleware integration.

- [ ] **Step 4: Write authorization.md**

Full authorization reference. Cover: gates (Define, Allows, Denies, Check, Any, None), policies (writing, registering, authorizing via gate), resource authorization, guest users.

- [ ] **Step 5: Write encryption-hashing.md**

Combined encryption + hashing reference. Cover: Encrypter facade (EncryptString, DecryptString), Hash facade (Make, Check, NeedsRehash), hashing drivers (Bcrypt, Argon2).

- [ ] **Step 6: Commit**

```bash
git add .claude/skills/goravel-security/
git commit -m "feat: add goravel-security skill — auth, authorization, encryption, hashing"
```

---

### Task 7: Generate goravel-async Skill

**Files:**
- Create: `.claude/skills/goravel-async/SKILL.md`
- Create: `.claude/skills/goravel-async/queues-reference.md`
- Create: `.claude/skills/goravel-async/events-reference.md`
- Create: `.claude/skills/goravel-async/scheduling-reference.md`
- Create: `.claude/skills/goravel-async/cache-reference.md`

**Source docs to read and distill:**
- `docs/goravel/en/digging-deeper/queues.md`
- `docs/goravel/en/digging-deeper/event.md`
- `docs/goravel/en/digging-deeper/task-scheduling.md`
- `docs/goravel/en/digging-deeper/cache.md`

- [ ] **Step 1: Read all source docs**

Read every file listed above. Focus on job dispatching, event/listener registration, schedule definitions, and cache operations.

- [ ] **Step 2: Write SKILL.md**

Create `.claude/skills/goravel-async/SKILL.md` with:

```markdown
---
name: goravel-async
description: Use when working with Goravel queues, events, listeners, task scheduling, or caching
---
```

Body should contain:
- **Overview:** Goravel's async/background processing layer
- **Core Patterns:**
  - Defining a job struct (implementing `queue.Job`)
  - Dispatching a job (`facades.Queue().Job().Dispatch()`)
  - Defining an event and listener
  - Registering events in `EventServiceProvider`
  - Scheduling a task (`schedule.Call()`, `schedule.Command()`)
  - Cache get/put (`facades.Cache().Get()`, `.Put()`)
- **Reference Files:** Index pointing to 4 supporting files

~200 lines max. Code-first. Imperative tone.

- [ ] **Step 3: Write queues-reference.md**

Full queues reference. Cover: job interface, dispatching (Dispatch, DispatchSync), job chains, delayed dispatch, retries, failed jobs, queue drivers (sync, redis, etc.), queue workers.

- [ ] **Step 4: Write events-reference.md**

Full events reference. Cover: defining events, defining listeners, registering events/listeners, dispatching events, queued listeners, event subscribers, event discovery.

- [ ] **Step 5: Write scheduling-reference.md**

Full scheduling reference. Cover: defining schedules (frequency methods: EveryMinute, Hourly, Daily, Weekly, Monthly, Cron), command scheduling, closure scheduling, preventing overlaps, running on one server, output handling.

- [ ] **Step 6: Write cache-reference.md**

Full cache reference. Cover: cache drivers (memory, redis, file, custom), Get, Put, Has, Forget, Flush, Forever, Remember, Pull, Increment, Decrement, tags, atomic locks, cache stores.

- [ ] **Step 7: Commit**

```bash
git add .claude/skills/goravel-async/
git commit -m "feat: add goravel-async skill — queues, events, scheduling, cache"
```

---

### Task 8: Generate goravel-tooling Skill

**Files:**
- Create: `.claude/skills/goravel-tooling/SKILL.md`
- Create: `.claude/skills/goravel-tooling/artisan-reference.md`
- Create: `.claude/skills/goravel-tooling/testing-reference.md`
- Create: `.claude/skills/goravel-tooling/logging-reference.md`

**Source docs to read and distill:**
- `docs/goravel/en/digging-deeper/artisan-console.md`
- `docs/goravel/en/digging-deeper/helpers.md`
- `docs/goravel/en/digging-deeper/processes.md`
- `docs/goravel/en/digging-deeper/strings.md`
- `docs/goravel/en/digging-deeper/color.md`
- `docs/goravel/en/digging-deeper/pluralization.md`
- `docs/goravel/en/testing/getting-started.md`
- `docs/goravel/en/testing/http-tests.md`
- `docs/goravel/en/testing/mock.md`
- `docs/goravel/en/the-basics/logging.md`

- [ ] **Step 1: Read all source docs**

Read every file listed above. Focus on artisan command creation, test setup, HTTP test assertions, mock facades, and logging channels.

- [ ] **Step 2: Write SKILL.md**

Create `.claude/skills/goravel-tooling/SKILL.md` with:

```markdown
---
name: goravel-tooling
description: Use when writing Goravel artisan commands, tests, or configuring logging
---
```

Body should contain:
- **Overview:** Goravel's developer tooling layer
- **Core Patterns:**
  - Creating an artisan command (struct implementing `console.Command`)
  - Registering commands in the Kernel
  - Writing a basic test with `testing` package
  - HTTP test basics (`Test().Get()`, `.Post()`, assertions)
  - Mocking facades in tests
  - Logging with `facades.Log()` (channels, levels)
  - Key helpers: `helpers.Env()`, string utilities
- **Reference Files:** Index pointing to 3 supporting files

~200 lines max. Code-first. Imperative tone.

- [ ] **Step 3: Write artisan-reference.md**

Full artisan reference. Cover: writing commands (Signature, Description, Handle), arguments and options, prompting for input, calling commands programmatically, output formatting (Info, Error, Line, Table), colors, progress bars, helper utilities (strings, pluralization, processes).

- [ ] **Step 4: Write testing-reference.md**

Full testing reference. Cover: test setup, HTTP tests (Get, Post, Put, Delete, assertions like AssertStatus, AssertJson, AssertSee), mocking (Mock facades, Mock methods), database testing (RefreshDatabase, factories, assertions), test lifecycle.

- [ ] **Step 5: Write logging-reference.md**

Full logging reference. Cover: log channels (single, daily, stack, custom), log levels (Emergency, Alert, Critical, Error, Warning, Notice, Info, Debug), contextual logging, writing to specific channels, creating custom drivers.

- [ ] **Step 6: Commit**

```bash
git add .claude/skills/goravel-tooling/
git commit -m "feat: add goravel-tooling skill — artisan, testing, logging"
```

---

### Task 9: Generate goravel-integrations Skill

**Files:**
- Create: `.claude/skills/goravel-integrations/SKILL.md`
- Create: `.claude/skills/goravel-integrations/grpc-reference.md`
- Create: `.claude/skills/goravel-integrations/http-client.md`
- Create: `.claude/skills/goravel-integrations/filesystem-reference.md`
- Create: `.claude/skills/goravel-integrations/mail-reference.md`

**Source docs to read and distill:**
- `docs/goravel/en/the-basics/grpc.md`
- `docs/goravel/en/digging-deeper/http-client.md`
- `docs/goravel/en/digging-deeper/filesystem.md`
- `docs/goravel/en/digging-deeper/mail.md`
- `docs/goravel/en/digging-deeper/localization.md`
- `docs/goravel/en/digging-deeper/package-development.md`

- [ ] **Step 1: Read all source docs**

Read every file listed above. Focus on gRPC server/client setup, HTTP client API, filesystem drivers, and mail sending.

- [ ] **Step 2: Write SKILL.md**

Create `.claude/skills/goravel-integrations/SKILL.md` with:

```markdown
---
name: goravel-integrations
description: Use when working with Goravel gRPC, HTTP client, filesystem, mail, or localization
---
```

Body should contain:
- **Overview:** Goravel's external integration layer
- **Core Patterns:**
  - Setting up a gRPC server and defining services
  - gRPC interceptors (unary, stream)
  - Making HTTP requests (`facades.Http().Get()`, `.Post()`)
  - Filesystem basics (`facades.Storage().Put()`, `.Get()`, `.Exists()`)
  - Sending mail (`facades.Mail().To().Content().Send()`)
  - Localization basics (`facades.Lang().Get()`)
  - Package development overview
- **Reference Files:** Index pointing to 4 supporting files

~200 lines max. Code-first. Imperative tone.

- [ ] **Step 3: Write grpc-reference.md**

Full gRPC reference. Cover: protobuf compilation, server setup, defining services, interceptors (unary/stream, server/client), client connections, streaming, error handling, metadata.

- [ ] **Step 4: Write http-client.md**

Full HTTP client reference. Cover: GET, POST, PUT, PATCH, DELETE, request headers, query parameters, request body (JSON, form), timeout, retries, concurrent requests, response handling.

- [ ] **Step 5: Write filesystem-reference.md**

Full filesystem reference. Cover: drivers (local, S3, custom), Put, Get, Exists, Delete, Move, Copy, directories, file metadata (Size, LastModified), temporary URLs, visibility, custom drivers.

- [ ] **Step 6: Write mail-reference.md**

Full mail reference. Cover: configuring mailers, sending mail (To, Cc, Bcc, Content, Attach, Send, Queue), markdown mailables, custom drivers, testing mail.

- [ ] **Step 7: Commit**

```bash
git add .claude/skills/goravel-integrations/
git commit -m "feat: add goravel-integrations skill — gRPC, HTTP client, filesystem, mail"
```

---

### Task 10: Cross-Reference Verification

**Files:**
- Modify: All 7 `SKILL.md` files (if cross-references are missing or incorrect)

- [ ] **Step 1: Check all SKILL.md files reference sibling skills correctly**

Read each SKILL.md and verify that:
- When a topic is covered in another skill, it says "See goravel-{other} skill" instead of duplicating
- Reference file paths are correct (relative within the skill directory)
- No skill covers a topic that belongs to another skill

- [ ] **Step 2: Verify no content duplication**

Grep across all skill files for key terms that should only appear in one skill:
- `facades.Route()` should only be detailed in goravel-http
- `facades.Orm()` should only be detailed in goravel-data
- `facades.Auth()` should only be detailed in goravel-security
- `facades.Queue()` should only be detailed in goravel-async

- [ ] **Step 3: Fix any issues found**

Update files as needed. Keep cross-references clean and consistent.

- [ ] **Step 4: Commit fixes (if any)**

```bash
git add .claude/skills/
git commit -m "fix: correct cross-references across goravel skills"
```

---

### Task 11: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add Goravel Skills section to CLAUDE.md**

Add after the existing content, before the `=== .ai/shadcn rules ===` section:

```markdown
## Goravel Skills

Goravel skills are available at `.claude/skills/goravel-*/`. Claude invokes them based on their descriptions when working on server code. For deeper API details, each skill's "Reference Files" section points to supporting files within the skill directory.

When working on the Go agent, these skills provide idiomatic Goravel patterns. Project-specific conventions (action-per-file, domain grouping, etc.) are defined above in this file and take precedence.
```

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add Goravel skills reference to CLAUDE.md"
```

---

### Task 12: Validate Skills with Retrieval Test

**Files:**
- No files created — this is a validation task

- [ ] **Step 1: Test retrieval — goravel-http**

Dispatch a subagent with this prompt (WITHOUT telling it about the skills):
> "I need to add a new POST endpoint to the Goravel server at `/api/containers/:id/restart` with JWT auth middleware. The handler should validate that `id` is required. Show me the route definition, controller method, and validation."

Verify the subagent finds and uses the goravel-http skill correctly.

- [ ] **Step 2: Test retrieval — goravel-data**

Dispatch a subagent with this prompt:
> "I need to create a new Goravel model called `AuditLog` with fields: action (string), user_id (uint), metadata (json), created_at. Include the migration file."

Verify the subagent finds and uses the goravel-data skill correctly.

- [ ] **Step 3: Test retrieval — goravel-foundations**

Dispatch a subagent with this prompt:
> "I need to create a new Goravel service provider that registers a `NotificationService` as a singleton in the container. Show me the provider and how to register it."

Verify the subagent finds and uses the goravel-foundations skill correctly.

- [ ] **Step 4: Document any gaps found**

If any skill failed to activate or had missing information, note the specific gap and fix it in the relevant skill file.

- [ ] **Step 5: Commit any fixes**

```bash
git add .claude/skills/
git commit -m "fix: address gaps found during skill validation"
```
