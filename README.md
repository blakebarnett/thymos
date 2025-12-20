# TLA - Technical Lead Assistance

Tools and documents for organizing Blue Team work within the CSO (Compute, Storage & Observability) division.

## Philosophy

> **Effort alignment, not task tracking.**

This repository contains tools to provide visibility into how Blue Team's work aligns with CSO 2026 initiatives, without imposing burdensome task-level tracking on an autonomous, self-motivated team.

## Contents

- [`docs/`](docs/) - Team guides and alignment documents
- [`scripts/`](scripts/) - CLI tools for status reporting
- [`templates/`](templates/) - Templates for updates, etc.

## Quick Start

```bash
# Add bin/ to your PATH (or symlink tla to somewhere in PATH)
export PATH="$PATH:$(pwd)/bin"

# Remember context from conversations
tla remember "Sean mentioned DCIM is blocked on vendor"

# Log decisions
tla decide "Using Cilium for DNS" --context "Evaluated CoreDNS"

# Prep for a 1:1
tla prep sean

# Search your memories
tla recall "what did we decide about DNS"
```

### Prerequisites

- [Locai CLI](https://github.com/your-org/locai) installed and in PATH
- Optionally: `LINEAR_API_KEY` for Linear integration

## CSO 2026 Initiatives

The Blue Team contributes primarily to these sub-initiatives:

| Sub-Initiative | Target | Blue Team Focus |
|----------------|--------|-----------------|
| Operational Excellence | Q1 2026 | Platform stability, automation |
| DC Bring-ups | Q1 2026 | Provisioning, DCIM |
| Improve Security Posture | Q1 2026 | TPM/SecureBoot, hardening |
| Reduction of Human Operations | Q1 2026 | Automation, self-healing |
| Platform Stability | Q1 2026 | SLOs, observability |
| V2 Enablement | Q1 2026 | V2 infrastructure support |

## Team

| Member | Primary Domains |
|--------|-----------------|
| Blake Barnett | Provisioning stack, Anodizer, FM |
| AJ Christensen | SRE generalist, security, automation |
| Michael Lang | Groq Node Controller, systems |
| Sean Cribbs | DCIM, SLO/SLI, observability |
| Clement Liaw (Jan 2025) | GPU deployments, DCIM |

