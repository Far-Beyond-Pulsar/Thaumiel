# Documentation index

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — how the crates fit together, the plugin registry, the domain model, and why several things are built the way they are rather than the more obvious alternative. Start here.
- **[PLUGINS.md](PLUGINS.md)** — writing a new keygen backend or auth provider, step by step, including a Windows-specific linker gotcha worth knowing about before you hit it.
- **[API.md](API.md)** — every HTTP route: auth requirements, request/response bodies, status codes.
- **[CONFIGURATION.md](CONFIGURATION.md)** — every config field, its environment variable name, and its default.
- **[DEPLOYMENT.md](DEPLOYMENT.md)** — Docker, what to change before this is a production deployment, key rotation behavior.

The top-level [`README.md`](../README.md) is the five-minute version of most of this; these go deeper on each piece.
