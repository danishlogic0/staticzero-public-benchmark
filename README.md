# staticzero-public-benchmark

## Repository scope

This repository is the reference point named in external technical documentation for
Danish Logic-0's benchmark and evidence material. It documents two things: how impact
is measured without access to a workload payload, and the boundary conditions under
which a bounded measurement is published at all.

This repository currently provides descriptive documentation of the impact-runner
approach and its publication boundary. It does not publish benchmark results or
open-source specification and verifier artefacts.

## Status of earlier published material

Those historical repository artefacts are no longer publicly available. They were made
private rather than left online without their complete boundary context, and this
repository does not claim to restore them. Public performance and bounded execution
claims remain suspended as an intentional evidence gate—not a permanent freeze—while
the underlying verification pipeline and evidence schemas are validated against the
exact criteria required before those bounded claims are resumed.

## Payload-blind impact runner

The impact runner is designed to measure execution-boundary behaviour without receiving
workload payloads: what a bounded operation does to its host, not what the workload
computes. Its measurement contract excludes reading, copying or retaining workload
payloads. Compliance with that contract is subject to the applicable verification and
evidence requirements.

What the measurement frame records is confined to the observable state of the execution
boundary itself — the resources an operation acquires and releases, any observed
persistence-surface activity within the declared measurement boundary, and the timing
of lifecycle transitions.

The method is paired measurement. The same workload is run inside the bounded corridor
and in a reference configuration outside it, on the same host and using the same
workload and runner build identity, and the two runs are compared against each other
rather than against a published figure. This describes the measurement method; it is
not a statement about the coverage of any earlier campaign. A comparison is meaningful
for that pair, on that host, under those pinned build identities.

The architectural requirement is that payload exists only in predeclared locations
during an operation, that those locations are cleared and verified before completion,
and that no payload remains anywhere after the operation. This requirement is distinct
from what any individual measurement has established.

This separation is why a party that must not receive a workload can still evaluate how
an operation behaved: the record describes the boundary, not the payload.

## Publication boundary

A measurement without its context is not a result another party can use, so none is
published here on its own. For a bounded measurement to be published as reference
material, it is accompanied by the build identity it was produced from, the hardware
architecture and kernel configuration of the host, the definition of the execution
boundary in force, the identity of the measured workload, the reference configuration
it was compared against, and the tooling needed to reproduce the comparison.

Summary phrasing that could be read outside the boundary it was measured in is not
published as an active claim. Statements about the architecture's design are kept
distinct from statements about what has been measured, and neither is presented as
external certification.

## Two distinct technical concerns

Work under Danish Logic-0™ separates two concerns.

**Bounded corridor execution (STATICZERO™)** — Danish-developed runtime software that is
designed to execute defined workloads inside isolated, single-use execution boundaries
in the operator's own environment, structured around pre-execution policy gating and
fail-closed supervision of the boundary.

**Vendor-neutral, payload-blind evidence verification** — specification and verification
work for deciding whether a signed runtime-evidence package is complete, internally
consistent and final, without inspecting workload payloads and without relying on a
proprietary runtime or a vendor control plane. The design is oriented toward the
concepts used in in-toto/DSSE, IETF RATS and IETF SCITT.

## References

- Website: https://staticzero.online
- Governance and architecture overview: https://staticzero.online/logic-0/
- Design properties and scoping: https://staticzero.online/evidence/
