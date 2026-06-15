# ADR Workflow

## Create A New ADR When

- a durable architecture or governance decision is being made
- meaningful alternatives were considered
- the consequences are expected to outlive the current implementation task

## Update An Existing ADR When

- the same decision needs status changes such as `accepted` or `superseded`
- new consequences are now known
- linked downstream work expands but does not replace the original decision

## Compact ADR Fields

- `title`
- `status`
- `date`
- `owner`
- `context`
- `decision`
- `alternatives`
- `consequences`
- `links`

## Filename Rule

Use `YYYY-MM-DD-slug.md`.

## Status Values

- `proposed`
- `accepted`
- `superseded`
- `rejected`
