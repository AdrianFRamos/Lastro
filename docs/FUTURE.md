# Planned expansions after the primitive

This document records product direction without turning hypotheses into hackathon features.

## Pilot Station

When a use case validates the need for location, the Station may incorporate GNSS with explicit provenance (`lat`, `lon`, accuracy, and time source). The choice between `StationEventV2` and additional evidence linked to `event_hash` depends on real auditability requirements; it is not frozen for the hackathon.

Secure provisioning, device registry, telemetry, field power, enclosure, and persistent retry also belong to the pilot phase.

## Establishments and trajectory

The next commercial step may add establishment identity, coordinates, origin/destination, and presence/movement associations. The objective is to produce a verifiable trajectory, not to declare compliance automatically.

## Integrations

PNIB, SISBOV, certifiers, slaughterhouses, exporters, and private systems are potential sources/consumers. Integrations must preserve external-data provenance and never convert imported data into “physical proof from the Station.”

## Compliance

Evidence packages and trajectory may support due-diligence processes, including workflows related to EUDR. A coordinate by itself is not equivalent to compliance.

## Risk & capital

Insurance, underwriting, credit, financing, and collateral enter only after commercial validation shows that verifiable identity/history reduces cost, fraud, reconciliation work, or risk uncertainty.

## On-chain physical assets

Collateral, securitization, and RWA require legal rights and enforceability beyond the cryptographic primitive. The order remains: identity → custody → history → economic applications.
