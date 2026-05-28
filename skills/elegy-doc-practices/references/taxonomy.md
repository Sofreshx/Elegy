# Documentation Taxonomy

## Types

- ADR: records a durable decision, alternatives, and consequences.
- Spec: records intended behavior, goals, non-goals, and validation.
- Guide: teaches recurring contributor or operator practice.
- Note: captures narrow context or local rationale with low governance weight.
- Roadmap: sequences future slices over time.

## ADR Or Spec

Use an ADR when the main question is "what durable decision are we making?"

Use a spec when the main question is "what behavior do we expect to implement or validate?"

Sometimes a change needs both:

- ADR for the durable decision
- spec for the detailed behavior slice

## Normal Notes

Do not escalate every change into an ADR or spec.

Prefer a normal note when:

- the scope is temporary or exploratory
- the change does not create a durable repo convention
- the behavior change is minor and already obvious from code plus tests
