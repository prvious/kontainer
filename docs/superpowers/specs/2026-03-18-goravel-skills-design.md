# Goravel Skills for Claude Code

**Date:** 2026-03-18
**Status:** Approved

## Goal

Create a comprehensive library of Claude Code skills that teach Claude to write idiomatic Goravel code. Skills are generic (not Kontainer-specific) and cover the full Goravel framework surface, organized by domain.

## Docs Integration

Pull the Goravel docs repo into the project via git subtree:

```bash
git subtree add --prefix=docs/goravel https://github.com/goravel/docs.git master --squash
```

This places the full docs repo at `docs/goravel/`. The `en/` subdirectory contains 48 relevant markdown files across 8 categories (architecture-concepts, database, digging-deeper, getting-started, orm, security, testing, the-basics) that serve as the source material for skill generation. Additional files exist in `prologue/` and `upgrade/` but are not used for skill content.

Update with: `git subtree pull --prefix=docs/goravel https://github.com/goravel/docs.git master --squash`

## Skill Architecture

### Layered Loading

Each domain is a **skill directory** with a `SKILL.md` entrypoint (~200 lines, Claude invokes based on `description`) and **supporting reference files** within the same directory (loaded on demand when Claude needs deeper detail):

```
.claude/
  skills/
    goravel-foundations/
      SKILL.md
      service-container.md
      service-providers.md
      facades.md
    goravel-http/
      SKILL.md
      routing-reference.md
      validation-rules.md
      request-response.md
    goravel-data/
      SKILL.md
      orm-methods.md
      query-builder.md
      migrations-reference.md
      relationships-advanced.md
    goravel-security/
      SKILL.md
      auth-guards.md
      authorization.md
      encryption-hashing.md
    goravel-async/
      SKILL.md
      queues-reference.md
      events-reference.md
      scheduling-reference.md
      cache-reference.md
    goravel-tooling/
      SKILL.md
      artisan-reference.md
      testing-reference.md
      logging-reference.md
    goravel-integrations/
      SKILL.md
      grpc-reference.md
      http-client.md
      filesystem-reference.md
      mail-reference.md
```

**Total: 7 skill directories, each with a SKILL.md + 3-4 reference files (31 files total).**

### SKILL.md Format

```markdown
---
name: goravel-{domain}
description: Use when {trigger description for this domain}
---

# Goravel {Domain}

## When to Use
[trigger conditions — what kind of task activates this skill]

## Core Patterns
[distilled, actionable patterns with Go code snippets for the 80% use case]

## Reference Files
For advanced usage, read the relevant reference file in this directory:
- `service-container.md` — one-line description
- `service-providers.md` — one-line description
```

### Reference File Format

Supporting files within each skill directory. Comprehensive API details — all method signatures, advanced patterns, edge cases. No frontmatter. Claude reads them on demand when the main SKILL.md points it there for a specific task.

## Skill Domains

### 1. goravel-foundations
**Description:** "Use when working with Goravel service container, service providers, facades, configuration, or application bootstrap"
**Source docs:** `en/architecture-concepts/` (facades, request-lifecycle, service-container, service-providers), `en/getting-started/` (compile, configuration, directory-structure, installation, packages)
**Core content:** Service container, service providers, facades, configuration, request lifecycle, directory structure
**Supporting refs:** service-container.md, service-providers.md, facades.md

### 2. goravel-http
**Description:** "Use when building HTTP endpoints, routes, controllers, middleware, request handling, response building, or validation in Goravel"
**Source docs:** `en/the-basics/` (routing, controllers, middleware, request, response, validation, session, views)
**Core content:** Routing basics, controllers, middleware, request/response, validation essentials
**Supporting refs:** routing-reference.md, validation-rules.md, request-response.md

### 3. goravel-data
**Description:** "Use when working with Goravel ORM models, database queries, migrations, seeding, or relationships"
**Source docs:** `en/database/` (getting-started, queries, migrations, seeding), `en/orm/` (getting-started, relationships, factories)
**Core content:** Defining models, basic CRUD, migrations basics, seeding
**Supporting refs:** orm-methods.md, query-builder.md, migrations-reference.md, relationships-advanced.md

### 4. goravel-security
**Description:** "Use when implementing authentication, authorization, encryption, or hashing in Goravel"
**Source docs:** `en/security/` (authentication, authorization, encryption, hashing)
**Core content:** Authentication guards, JWT basics, authorization gates/policies, hashing
**Supporting refs:** auth-guards.md, authorization.md, encryption-hashing.md

### 5. goravel-async
**Description:** "Use when working with Goravel queues, events, listeners, task scheduling, or caching"
**Source docs:** `en/digging-deeper/` (queues, event, task-scheduling, cache)
**Core content:** Queues basics, events/listeners pattern, task scheduling, cache usage
**Supporting refs:** queues-reference.md, events-reference.md, scheduling-reference.md, cache-reference.md

### 6. goravel-tooling
**Description:** "Use when writing Goravel artisan commands, tests, or configuring logging"
**Source docs:** `en/testing/` (getting-started, http-tests, mock), `en/digging-deeper/` (artisan-console, helpers, processes, strings, color, pluralization), `en/the-basics/logging`
**Core content:** Artisan commands, testing basics, logging essentials, helper utilities
**Supporting refs:** artisan-reference.md, testing-reference.md, logging-reference.md

### 7. goravel-integrations
**Description:** "Use when working with Goravel gRPC, HTTP client, filesystem, mail, or localization"
**Source docs:** `en/the-basics/grpc`, `en/digging-deeper/` (http-client, filesystem, mail, localization, package-development)
**Core content:** gRPC setup, HTTP client basics, filesystem basics
**Supporting refs:** grpc-reference.md, http-client.md, filesystem-reference.md, mail-reference.md

## Content Principles

- **Imperative tone** — "Use `facades.Route().Get()` to define a GET route" not "Routes can be defined using..."
- **Code-first** — Lead with Go code snippets, explain after
- **Goravel v1.17** — Pinned to the version in go.mod. Note any version-specific APIs when the subtree docs describe features beyond v1.17.
- **No duplication across skills** — Cross-reference other skills instead of repeating content
- **Distilled, not copied** — Rewrite docs as actionable instruction, not verbatim reproduction
- **~200 lines per SKILL.md** — Keep main skill files lean; push comprehensive details to supporting reference files

## CLAUDE.md Integration

Add a lightweight section to CLAUDE.md:

```markdown
## Goravel Skills

Goravel skills are available at `.claude/skills/goravel-*/`. Claude invokes them based on their descriptions when working on server code. For deeper API details, each skill's "Reference Files" section points to supporting files within the skill directory.

When working on the Go agent, these skills provide idiomatic Goravel patterns. Project-specific conventions (action-per-file, domain grouping, etc.) are defined above in this file and take precedence.
```

## Execution Order

1. `git subtree add` the Goravel docs into `docs/goravel/`
2. Create skill directory structure at `.claude/skills/goravel-*/`
3. Generate `goravel-foundations` first (other skills reference service providers/facades)
4. Generate `goravel-http` and `goravel-data` next (most frequently used)
5. Generate remaining skills: `goravel-security`, `goravel-async`, `goravel-tooling`, `goravel-integrations`
6. Final pass: verify cross-references between skills are correct
7. Update CLAUDE.md with the Goravel Skills section
