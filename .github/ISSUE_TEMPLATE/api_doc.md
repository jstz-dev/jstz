---
name: 📚 Web/runtime APIs documentation task
about: Ticket for implementing a documentation for Web/runtime API
title: "📚 Docs: <title>"
labels: ["documentation", "jstz::api"]
assignees: ""
---

# Context

**Specification**: [Name](link) \
**Specification interface**: [Interface name](link)

Additional documentation:

- [mdn docs](link)
- [`deno` docs](link)
- [Cloudflare workers docs](link)
- [`jstz` docs](https://trilitech.github.io/jstz/)

The documentation is to be added to the `jstz` vitepress documentation site.
To edit documentation:

- Find or add a documentation file in `docs/api/`
- Modify documentation in markdown
- Locally test the documentation (with live reload) using
  ```sh
  npm run docs:dev
  ```
